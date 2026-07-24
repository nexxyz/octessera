use octessera_hal::board_profiles::RASPBERRY_PI_ZERO_2W_ID;
use playback_runtime::RuntimeSystemInfo;
use std::net::UdpSocket;

pub(super) fn collect() -> Result<RuntimeSystemInfo, String> {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .or_else(|_| std::env::var("HOSTNAME").map_err(std::io::Error::other))
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "unavailable".into());
    Ok(RuntimeSystemInfo {
        os: std::env::consts::OS.into(),
        os_version: os_version(),
        octessera_version: env!("CARGO_PKG_VERSION").into(),
        primary_ip: primary_ip(),
        primary_mac: primary_mac(),
        hostname,
        board_profile: RASPBERRY_PI_ZERO_2W_ID.into(),
    })
}

fn os_version() -> String {
    if let Ok(release) = std::fs::read_to_string("/etc/os-release") {
        if let Some(name) = release
            .lines()
            .find_map(|line| line.strip_prefix("PRETTY_NAME=").map(unquote))
        {
            return name;
        }
    }
    std::process::Command::new("uname")
        .args(["-sr"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".into())
}

fn unquote(value: &str) -> String {
    value.trim_matches('"').to_string()
}

fn primary_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_loopback()).then(|| address.to_string())
}

fn primary_mac() -> Option<String> {
    let route = std::fs::read_to_string("/proc/net/route").ok()?;
    let interface = route.lines().skip(1).find_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        (fields.get(1) == Some(&"00000000"))
            .then(|| fields.first().map(|field| (*field).to_string()))
    })??;
    let address = std::fs::read_to_string(format!("/sys/class/net/{interface}/address")).ok()?;
    let address = address.trim();
    is_mac(address).then(|| address.to_string())
}

fn is_mac(value: &str) -> bool {
    let octets = value.split([':', '-']).collect::<Vec<_>>();
    octets.len() == 6
        && octets.iter().all(|octet| {
            octet.len() == 2 && octet.chars().all(|character| character.is_ascii_hexdigit())
        })
}

#[cfg(test)]
mod tests {
    use super::is_mac;

    #[test]
    fn accepts_only_mac_addresses() {
        assert!(is_mac("aa:bb:cc:dd:ee:ff"));
        assert!(!is_mac("not-a-mac"));
    }
}
