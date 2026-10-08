//! System > Setup > Show Hold: records when postponable board maintenance may
//! run again. The board images give fstrim, logrotate, dpkg-db-backup,
//! tmpfiles-clean (and the Raspberry network logger) an `ExecCondition` that
//! skips them while this time is in the future.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const HOLD_FILE: &str = "/var/lib/octessera/show-hold/until";
const HOLD_DURATION: Duration = Duration::from_secs(48 * 60 * 60);

pub(crate) fn set_hold(active: bool) -> Result<(), String> {
    set_hold_at(Path::new(HOLD_FILE), active, SystemTime::now())
}

fn set_hold_at(path: &Path, active: bool, now: SystemTime) -> Result<(), String> {
    if !active {
        return match std::fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.to_string()),
            _ => Ok(()),
        };
    }
    let until = (now + HOLD_DURATION)
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    std::fs::write(path, format!("{until}\n")).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::set_hold_at;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn hold_writes_the_expiry_and_release_removes_it() {
        let dir = std::env::temp_dir().join(format!("octessera-show-hold-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("until");
        let now = UNIX_EPOCH + Duration::from_secs(1_000);

        set_hold_at(&path, true, now).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "173800\n");
        set_hold_at(&path, false, now).unwrap();
        assert!(!path.exists());
        set_hold_at(&path, false, now).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
