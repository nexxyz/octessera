//! System > Live Show Mode and Connections > Wi-Fi: the runtime records both board
//! switches as files that root's `octessera-show-hold-apply` acts on. The show
//! file holds the show's length, not a wall-clock expiry: root counts it down on
//! the monotonic clock across reboots and removes the file when the show is
//! over. While it exists, fstrim, logrotate, tmpfiles-clean (and the Raspberry
//! network logger) are skipped and the radio is blocked; the Wi-Fi switch turns
//! NetworkManager's radio off until it is turned back on. The two are
//! independent: a show never changes the Wi-Fi switch.

use std::path::Path;
use std::time::Duration;

const HOLD_FILE: &str = "/var/lib/octessera/show-hold/length";
const WIFI_OFF_FILE: &str = "/var/lib/octessera/show-hold/wifi-off";
const HOLD_DURATION: Duration = Duration::from_secs(48 * 60 * 60);

pub(crate) fn set_hold(active: bool) -> Result<(), String> {
    set_hold_at(Path::new(HOLD_FILE), active)
}

pub(crate) fn set_wifi_enabled(enabled: bool) -> Result<(), String> {
    set_wifi_enabled_at(Path::new(WIFI_OFF_FILE), enabled)
}

fn set_wifi_enabled_at(path: &Path, enabled: bool) -> Result<(), String> {
    if enabled {
        remove_if_present(path)
    } else {
        std::fs::write(
            path, "off
",
        )
        .map_err(|error| error.to_string())
    }
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.to_string()),
        _ => Ok(()),
    }
}

fn set_hold_at(path: &Path, active: bool) -> Result<(), String> {
    if !active {
        return remove_if_present(path);
    }
    std::fs::write(path, format!("{}\n", HOLD_DURATION.as_secs()))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{set_hold_at, set_wifi_enabled_at};

    #[test]
    fn hold_writes_the_show_length_and_release_removes_it() {
        let dir = std::env::temp_dir().join(format!("octessera-show-hold-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("length");

        set_hold_at(&path, true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "172800\n");
        set_hold_at(&path, false).unwrap();
        assert!(!path.exists());
        set_hold_at(&path, false).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn wifi_off_writes_the_switch_and_on_removes_it() {
        let dir =
            std::env::temp_dir().join(format!("octessera-wifi-switch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wifi-off");

        set_wifi_enabled_at(&path, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "off
"
        );
        set_wifi_enabled_at(&path, true).unwrap();
        assert!(!path.exists());
        set_wifi_enabled_at(&path, true).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
