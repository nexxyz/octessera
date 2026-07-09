use std::fs;
use std::fs::OpenOptions;
use std::path::Path;
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

    for (label, result) in [
        (
            "config.txt exists",
            check_path_exists(Path::new("/boot/firmware/config.txt")),
        ),
        (
            "config.txt boot settings",
            check_config_settings(Path::new("/boot/firmware/config.txt")),
        ),
        (
            "/dev/i2c-1 read/write",
            check_device_read_write(Path::new("/dev/i2c-1")),
        ),
        (
            "/dev/spidev0.0 read/write",
            check_device_read_write(Path::new("/dev/spidev0.0")),
        ),
        ("pinctrl GPIO14/15 inputs", check_pinctrl_gpio14_gpio15()),
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
        "enable_uart=0",
        "dtoverlay=i2s-dac-no20",
    ];
    let missing = required
        .iter()
        .copied()
        .filter(|needle| !content.contains(needle))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("missing {}", missing.join(", ")))
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
