#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::process::Command;

pub fn profile_system_output() -> Vec<(String, String)> {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        orange_system_output()
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    {
        vcgencmd_output()
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn vcgencmd_output() -> Vec<(String, String)> {
    ["measure_temp", "get_throttled"]
        .into_iter()
        .filter_map(|metric| run_command("vcgencmd", &[metric]).map(|value| (metric.into(), value)))
        .collect()
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn orange_system_output() -> Vec<(String, String)> {
    let mut output = vec![(
        "board_profile".to_string(),
        octessera_pi::board_profile::BOARD_PROFILE_ID.to_string(),
    )];
    if let Some(value) = read_system_file("/proc/loadavg") {
        output.push(("loadavg".into(), value));
    }
    if let Some(value) = read_mem_available() {
        output.push(("mem_available_kb".into(), value));
    }
    if let Some(value) = read_system_file("/sys/class/thermal/thermal_zone0/temp") {
        output.push(("thermal_zone0_millicelsius".into(), value));
    }
    if let Some(value) = read_system_file("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq") {
        output.push(("cpu0_scaling_cur_freq_khz".into(), value));
    }
    output
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn read_system_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn read_mem_available() -> Option<String> {
    std::fs::read_to_string("/proc/meminfo")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("MemAvailable:")?
                .split_whitespace()
                .next()
        })
        .map(str::to_string)
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn run_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
