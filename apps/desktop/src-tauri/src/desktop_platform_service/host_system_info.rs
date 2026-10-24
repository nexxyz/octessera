use playback_runtime::RuntimeSystemInfo;
use std::net::UdpSocket;

pub(super) fn collect_system_info() -> Result<RuntimeSystemInfo, String> {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unavailable".into());
    let primary_ip = primary_ip();
    let primary_mac = primary_mac();
    Ok(RuntimeSystemInfo {
        os: std::env::consts::OS.into(),
        os_version: os_version(),
        octessera_version: env!("CARGO_PKG_VERSION").into(),
        primary_ip,
        primary_mac,
        hostname,
        board_profile: "desktop-simulator".into(),
    })
}

fn os_version() -> String {
    #[cfg(target_os = "windows")]
    let command = ("cmd", vec!["/C", "ver"]);
    #[cfg(not(target_os = "windows"))]
    let command = ("uname", vec!["-sr"]);
    std::process::Command::new(command.0)
        .args(command.1)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "unavailable".into())
}

fn primary_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_loopback()).then(|| address.to_string())
}

fn primary_mac() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("getmac")
            .args(["/fo", "csv", "/nh"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout)
            .split(|character: char| {
                !character.is_ascii_hexdigit() && character != '-' && character != ':'
            })
            .find(|value| is_mac(value))
            .map(ToOwned::to_owned)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut interfaces = std::fs::read_dir("/sys/class/net")
            .ok()?
            .flatten()
            .collect::<Vec<_>>();
        interfaces.sort_by_key(|entry| entry.file_name());
        for interface in interfaces {
            if interface.file_name() == "lo" {
                continue;
            }
            let Ok(address) = std::fs::read_to_string(interface.path().join("address")) else {
                continue;
            };
            let address = address.trim();
            if is_mac(address) {
                return Some(address.to_string());
            }
        }
        None
    }
}

fn is_mac(value: &str) -> bool {
    let octets = value.split([':', '-']).collect::<Vec<_>>();
    octets.len() == 6
        && octets
            .iter()
            .all(|octet| octet.len() == 2 && octet.chars().all(|c| c.is_ascii_hexdigit()))
}
