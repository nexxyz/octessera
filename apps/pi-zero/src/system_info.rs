use crate::board_profile::BOARD_PROFILE_ID;
use playback_runtime::RuntimeSystemInfo;
use std::net::Ipv4Addr;

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct InterfaceIpv4Candidate {
    interface: String,
    address: Ipv4Addr,
    netmask: Ipv4Addr,
    interface_up: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RegularWlan0Ipv4 {
    pub(crate) address: Ipv4Addr,
    pub(crate) netmask: Ipv4Addr,
}

impl RegularWlan0Ipv4 {
    pub(crate) fn network(self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.address) & u32::from(self.netmask))
    }

    pub(crate) fn contains(self, address: Ipv4Addr) -> bool {
        u32::from(address) & u32::from(self.netmask) == u32::from(self.network())
    }
}

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
        board_profile: BOARD_PROFILE_ID.into(),
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

#[cfg(target_os = "linux")]
fn primary_ip() -> Option<String> {
    regular_wlan0_ipv4()
        .ok()
        .map(|network| network.address.to_string())
}

#[cfg(not(target_os = "linux"))]
fn primary_ip() -> Option<String> {
    None
}

pub(crate) fn regular_wlan0_ipv4() -> Result<RegularWlan0Ipv4, String> {
    #[cfg(target_os = "linux")]
    {
        return select_wlan0_ipv4(&interface_ipv4_candidates())
            .ok_or_else(|| "regular wlan0 IPv4 address is unavailable".into());
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("regular wlan0 IPv4 address is unavailable on this platform".into())
    }
}

#[cfg(target_os = "linux")]
fn interface_ipv4_candidates() -> Vec<InterfaceIpv4Candidate> {
    use std::ffi::CStr;

    let mut addresses = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut addresses) } != 0 {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let mut current = addresses;
    while !current.is_null() {
        unsafe {
            let interface = &*current;
            if !interface.ifa_name.is_null() && !interface.ifa_addr.is_null() {
                let address = interface.ifa_addr;
                if (*address).sa_family as libc::c_int == libc::AF_INET {
                    let sockaddr = &*(address as *const libc::sockaddr_in);
                    let netmask = interface.ifa_netmask;
                    if netmask.is_null() {
                        current = interface.ifa_next;
                        continue;
                    }
                    let netmask = &*(netmask as *const libc::sockaddr_in);
                    let flags = interface.ifa_flags as libc::c_uint;
                    candidates.push(InterfaceIpv4Candidate {
                        interface: CStr::from_ptr(interface.ifa_name)
                            .to_string_lossy()
                            .into_owned(),
                        address: Ipv4Addr::from(u32::from_be(sockaddr.sin_addr.s_addr)),
                        netmask: Ipv4Addr::from(u32::from_be(netmask.sin_addr.s_addr)),
                        interface_up: flags & libc::IFF_UP as libc::c_uint != 0,
                    });
                }
            }
            current = interface.ifa_next;
        }
    }
    unsafe { libc::freeifaddrs(addresses) };
    candidates
}

#[cfg(any(target_os = "linux", test))]
fn select_wlan0_ipv4(candidates: &[InterfaceIpv4Candidate]) -> Option<RegularWlan0Ipv4> {
    candidates
        .iter()
        .find(|candidate| {
            candidate.interface == "wlan0"
                && candidate.interface_up
                && is_usable_ipv4(candidate.address)
                && is_contiguous_netmask(candidate.netmask)
                && is_usable_host_address(candidate.address, candidate.netmask)
        })
        .map(|candidate| RegularWlan0Ipv4 {
            address: candidate.address,
            netmask: candidate.netmask,
        })
}

#[cfg(any(target_os = "linux", test))]
fn is_usable_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    address != Ipv4Addr::UNSPECIFIED
        && !address.is_loopback()
        && address != Ipv4Addr::new(192, 168, 42, 1)
        && octets[..2] != [169, 254]
        && !(octets[0] == 0
            || (octets[0] == 100 && (64..=127).contains(&octets[1]))
            || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
            || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
            || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
            || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
            || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
            || octets[0] >= 224)
}

#[cfg(any(target_os = "linux", test))]
fn is_contiguous_netmask(netmask: Ipv4Addr) -> bool {
    let host_bits = !u32::from(netmask);
    host_bits & host_bits.wrapping_add(1) == 0
}

#[cfg(any(target_os = "linux", test))]
fn is_usable_host_address(address: Ipv4Addr, netmask: Ipv4Addr) -> bool {
    if netmask == Ipv4Addr::BROADCAST {
        return true;
    }
    let network = u32::from(address) & u32::from(netmask);
    let broadcast = network | !u32::from(netmask);
    u32::from(address) != network && u32::from(address) != broadcast
}

#[cfg(target_os = "linux")]
fn primary_mac() -> Option<String> {
    regular_wlan0_ipv4().ok()?;
    let address = std::fs::read_to_string("/sys/class/net/wlan0/address").ok()?;
    let address = address.trim();
    is_mac(address).then(|| address.to_string())
}

#[cfg(not(target_os = "linux"))]
fn primary_mac() -> Option<String> {
    None
}

#[cfg(any(target_os = "linux", test))]
fn is_mac(value: &str) -> bool {
    let octets = value.split([':', '-']).collect::<Vec<_>>();
    octets.len() == 6
        && octets.iter().all(|octet| {
            octet.len() == 2 && octet.chars().all(|character| character.is_ascii_hexdigit())
        })
}

#[cfg(test)]
mod tests {
    use super::{
        is_contiguous_netmask, is_mac, is_usable_host_address, is_usable_ipv4, select_wlan0_ipv4,
        InterfaceIpv4Candidate,
    };
    use std::net::Ipv4Addr;

    fn candidate(interface: &str, address: &str, interface_up: bool) -> InterfaceIpv4Candidate {
        InterfaceIpv4Candidate {
            interface: interface.into(),
            address: address.parse().unwrap(),
            netmask: "255.255.255.0".parse().unwrap(),
            interface_up,
        }
    }

    #[test]
    fn accepts_only_mac_addresses() {
        assert!(is_mac("aa:bb:cc:dd:ee:ff"));
        assert!(!is_mac("not-a-mac"));
    }

    #[test]
    fn accepts_private_and_public_ipv4_addresses() {
        assert!(is_usable_ipv4(Ipv4Addr::new(192, 168, 1, 20)));
        assert!(is_usable_ipv4(Ipv4Addr::new(8, 8, 8, 8)));
    }

    #[test]
    fn rejects_non_usable_ipv4_addresses() {
        for address in [
            Ipv4Addr::UNSPECIFIED,
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(169, 254, 10, 20),
            Ipv4Addr::new(192, 168, 42, 1),
            Ipv4Addr::new(192, 0, 2, 10),
            Ipv4Addr::new(198, 18, 1, 10),
            Ipv4Addr::new(224, 0, 0, 1),
            Ipv4Addr::new(240, 0, 0, 1),
        ] {
            assert!(!is_usable_ipv4(address));
        }
    }

    #[test]
    fn selects_an_up_usable_wlan0_address_over_other_interfaces() {
        let candidates = [
            candidate("eth0", "10.0.0.4", true),
            candidate("wlan0", "192.168.1.20", true),
            candidate("wlan0", "192.168.1.21", false),
        ];
        assert_eq!(
            select_wlan0_ipv4(&candidates),
            Some(super::RegularWlan0Ipv4 {
                address: Ipv4Addr::new(192, 168, 1, 20),
                netmask: Ipv4Addr::new(255, 255, 255, 0),
            })
        );
    }

    #[test]
    fn does_not_select_another_interface_when_wlan0_is_unavailable() {
        let candidates = [candidate("eth0", "10.0.0.4", true)];
        assert_eq!(select_wlan0_ipv4(&candidates), None);
    }

    #[test]
    fn rejects_non_contiguous_netmasks() {
        assert!(is_contiguous_netmask(Ipv4Addr::new(255, 255, 255, 0)));
        assert!(!is_contiguous_netmask(Ipv4Addr::new(255, 255, 0, 255)));
    }

    #[test]
    fn rejects_network_and_broadcast_addresses_but_accepts_host_addresses() {
        let mask = Ipv4Addr::new(255, 255, 255, 0);
        assert!(!is_usable_host_address(Ipv4Addr::new(192, 168, 1, 0), mask));
        assert!(!is_usable_host_address(
            Ipv4Addr::new(192, 168, 1, 255),
            mask
        ));
        assert!(is_usable_host_address(Ipv4Addr::new(192, 168, 1, 20), mask));
        assert!(is_usable_host_address(
            Ipv4Addr::new(192, 168, 1, 20),
            Ipv4Addr::BROADCAST
        ));
    }

    #[test]
    fn rejects_setup_gateway_from_wlan0_selection() {
        let candidates = [candidate("wlan0", "192.168.42.1", true)];
        assert_eq!(select_wlan0_ipv4(&candidates), None);
    }

    #[test]
    fn computes_regular_wlan0_subnet_membership_from_netmask() {
        let network = super::RegularWlan0Ipv4 {
            address: Ipv4Addr::new(192, 168, 1, 20),
            netmask: Ipv4Addr::new(255, 255, 255, 0),
        };
        assert!(network.contains(Ipv4Addr::new(192, 168, 1, 99)));
        assert!(!network.contains(Ipv4Addr::new(192, 168, 2, 20)));
    }
}
