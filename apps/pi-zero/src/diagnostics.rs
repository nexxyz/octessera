use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn diagnostic_requested() -> bool {
    if std::env::args().skip(1).any(|arg| arg == "--diagnostic") {
        return true;
    }
    std::env::var("OCTESSERA_PI_DIAGNOSTIC")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "preflight" | "hardware"
            )
        })
}

pub fn run_pre_hardware_diagnostics() -> bool {
    if !cfg!(target_os = "linux") {
        println!("FAIL diagnostics: Linux host required");
        return false;
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    let boot_directory = match find_boot_directory() {
        Ok(path) => path,
        Err(error) => {
            println!("FAIL diagnostics: {error}");
            return false;
        }
    };

    for (label, result) in [
        (
            "config.txt exists",
            check_path_exists(&boot_directory.join("config.txt")),
        ),
        (
            "config.txt boot settings",
            check_config_settings(&boot_directory.join("config.txt")),
        ),
        (
            "cmdline.txt serial console settings",
            check_cmdline_settings(&boot_directory.join("cmdline.txt")),
        ),
        (
            "/dev/i2c-1 read/write",
            check_device_read_write(Path::new("/dev/i2c-1")),
        ),
        (
            "/dev/spidev0.0 read/write",
            check_device_read_write(Path::new("/dev/spidev0.0")),
        ),
        (
            "inactive UART safety and GPIO14/15 inputs",
            check_pinctrl_gpio14_gpio15(),
        ),
        ("pinctrl PCM pins", check_pinctrl_pcm_pins()),
        ("aplay -l DAC", check_aplay_dac_listing()),
    ] {
        if report_check(label, result) {
            passed += 1;
        } else {
            failed += 1;
        }
    }

    if failed == 0 {
        println!("PASS diagnostics complete ({passed} checks)");
        true
    } else {
        println!("FAIL diagnostics complete ({failed} failed)");
        false
    }
}

fn find_boot_directory() -> Result<PathBuf, String> {
    [Path::new("/boot/firmware"), Path::new("/boot")]
        .into_iter()
        .find(|directory| {
            directory.join("config.txt").is_file() && directory.join("cmdline.txt").is_file()
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| "matching boot directory with config.txt and cmdline.txt not found".into())
}

fn report_check(label: &str, result: Result<(), String>) -> bool {
    match result {
        Ok(()) => {
            println!("PASS {label}");
            true
        }
        Err(message) => {
            println!("FAIL {label}: {message}");
            false
        }
    }
}

fn check_path_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        Ok(())
    } else {
        Err(format!("{} not found", path.display()))
    }
}

fn check_config_settings(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|error| format!("{error}"))?;
    let required = [
        "dtparam=i2c_arm=on",
        "dtparam=spi=on",
        "dtparam=audio=off",
        "dtoverlay=i2s-dac-no20",
    ];
    let missing = required
        .iter()
        .copied()
        .filter(|needle| !content.contains(needle))
        .collect::<Vec<_>>();
    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!("missing {}", missing.join(", ")));
    }

    let active_lines = content.lines().filter_map(active_config_line);
    let mut uart_values = Vec::new();
    let mut disable_bt_count = 0usize;
    for line in active_lines {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "enable_uart" {
            uart_values.push(value);
        } else if key == "dtoverlay" && value == "disable-bt" {
            disable_bt_count += 1;
        }
    }

    if uart_values.len() != 1 || uart_values.first().copied() != Some("0") {
        if uart_values.is_empty() {
            problems.push("missing active enable_uart=0".into());
        } else {
            problems.push(format!(
                "expected exactly one active enable_uart=0, found {}",
                uart_values.join(", ")
            ));
        }
    }
    if disable_bt_count != 1 {
        problems.push(format!(
            "expected exactly one active dtoverlay=disable-bt, found {disable_bt_count}"
        ));
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

fn active_config_line(line: &str) -> Option<&str> {
    let line = line.split('#').next()?.trim();
    (!line.is_empty()).then_some(line)
}

fn check_cmdline_settings(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|error| format!("{error}"))?;
    let forbidden_aliases = ["serial0", "ttyAMA0", "ttyS0"];
    let present = content
        .split_whitespace()
        .filter_map(|token| token.strip_prefix("console="))
        .filter_map(|value| value.split(',').next())
        .filter(|alias| forbidden_aliases.contains(alias))
        .collect::<Vec<_>>();
    if present.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "UART console tokens present: {}",
            present.join(", ")
        ))
    }
}

fn check_device_read_write(path: &Path) -> Result<(), String> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| format!("{error}"))
}

fn check_pinctrl_gpio14_gpio15() -> Result<(), String> {
    let output_14 = run_command("pinctrl", &["get", "14"])?;
    let output_15 = run_command("pinctrl", &["get", "15"])?;
    require_contains("GPIO14 = input", &output_14, "GPIO14 = input", "GPIO14")?;
    require_contains("GPIO15 = input", &output_15, "GPIO15 = input", "GPIO15")
}

fn check_pinctrl_pcm_pins() -> Result<(), String> {
    let output_18 = run_command("pinctrl", &["get", "18"])?;
    let output_19 = run_command("pinctrl", &["get", "19"])?;
    let output_20 = run_command("pinctrl", &["get", "20"])?;
    let output_21 = run_command("pinctrl", &["get", "21"])?;
    require_contains("PCM_CLK", &output_18, "PCM_CLK", "GPIO18")?;
    require_contains("PCM_FS", &output_19, "PCM_FS", "GPIO19")?;
    require_contains("GPIO20 = input", &output_20, "GPIO20 = input", "GPIO20")?;
    require_contains("PCM_DOUT", &output_21, "PCM_DOUT", "GPIO21")
}

fn check_aplay_dac_listing() -> Result<(), String> {
    let output = run_command("aplay", &["-l"])?;
    let lower = output.to_ascii_lowercase();
    if lower.contains("hifiberry")
        || lower.contains("pcm5102a")
        || lower.contains("snd_rpi_hifiberry")
    {
        Ok(())
    } else {
        Err("missing HifiBerry/pcm5102a/snd_rpi_hifiberry entry".into())
    }
}

fn require_contains(
    needle_label: &str,
    haystack: &str,
    needle: &str,
    pin_label: &str,
) -> Result<(), String> {
    if haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
    {
        Ok(())
    } else {
        Err(format!(
            "{pin_label} missing {needle_label}: {}",
            trim_output(haystack)
        ))
    }
}

fn run_command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("{program} unavailable: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format_command_failure(
            program,
            args,
            &output.stdout,
            &output.stderr,
        ))
    }
}

fn format_command_failure(program: &str, args: &[&str], stdout: &[u8], stderr: &[u8]) -> String {
    let mut parts = Vec::new();
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if !stdout.is_empty() {
        parts.push(format!("stdout: {stdout}"));
    }
    if !stderr.is_empty() {
        parts.push(format!("stderr: {stderr}"));
    }
    if parts.is_empty() {
        parts.push("no output".into());
    }
    format!("{program} {} failed ({})", args.join(" "), parts.join("; "))
}

fn trim_output(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 120 {
        trimmed.into()
    } else {
        format!("{}...", trimmed.chars().take(120).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::{check_cmdline_settings, check_config_settings};
    use std::fs;
    use std::path::PathBuf;

    fn temporary_file(name: &str, contents: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "octessera-diagnostics-{}-{}",
            std::process::id(),
            name
        ));
        fs::create_dir_all(&directory).expect("temporary diagnostics directory");
        let path = directory.join(name);
        fs::write(&path, contents).expect("temporary diagnostics file");
        path
    }

    fn valid_config() -> &'static str {
        "dtparam=i2c_arm=on\ndtparam=spi=on\ndtparam=audio=off\nenable_uart=0\ndtoverlay=disable-bt\ndtoverlay=i2s-dac-no20\n"
    }

    #[test]
    fn config_requires_uart_disable_and_bluetooth_overlay() {
        let path = temporary_file("config.txt", valid_config());
        assert!(check_config_settings(&path).is_ok());

        let missing_overlay = valid_config().replace("dtoverlay=disable-bt\n", "");
        fs::write(&path, missing_overlay).expect("write config fixture");
        assert!(check_config_settings(&path).is_err());
    }

    #[test]
    fn config_ignores_commented_uart_settings() {
        let path = temporary_file(
            "config-comments.txt",
            &valid_config()
                .replace("enable_uart=0", "# enable_uart=0")
                .replace("dtoverlay=disable-bt", "# dtoverlay=disable-bt"),
        );
        assert!(check_config_settings(&path).is_err());
    }

    #[test]
    fn config_rejects_duplicate_uart_settings() {
        let path = temporary_file(
            "config-duplicate-uart.txt",
            &format!("{}enable_uart=0\n", valid_config()),
        );
        assert!(check_config_settings(&path).is_err());
    }

    #[test]
    fn config_rejects_conflicting_uart_settings() {
        let path = temporary_file(
            "config-conflicting-uart.txt",
            &valid_config().replace("enable_uart=0", "enable_uart=1"),
        );
        assert!(check_config_settings(&path).is_err());
    }

    #[test]
    fn config_rejects_near_uart_values() {
        let path = temporary_file(
            "config-near-uart.txt",
            &valid_config().replace("enable_uart=0", "enable_uart=01"),
        );
        assert!(check_config_settings(&path).is_err());
    }

    #[test]
    fn cmdline_rejects_exact_console_aliases_with_whitespace() {
        let path = temporary_file("cmdline.txt", "root=/dev/mmcblk0p2\tconsole=ttyS0\n");
        assert!(check_cmdline_settings(&path).is_err());

        fs::write(
            &path,
            "console=ttyS0 console=serial0,115200 console=ttyAMA0",
        )
        .expect("write cmdline fixture");
        assert!(check_cmdline_settings(&path).is_err());
    }

    #[test]
    fn cmdline_allows_near_console_matches() {
        let path = temporary_file(
            "cmdline-near-match.txt",
            "xconsole=serial0 console=serial00 console=ttyS00 console=notserial0",
        );
        assert!(check_cmdline_settings(&path).is_ok());
    }
}
