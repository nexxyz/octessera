use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const HEALTH_PATH_ENV: &str = "OCTESSERA_CANDIDATE_HEALTH_PATH";
const INVOCATION_ID_ENV: &str = "INVOCATION_ID";
const MAX_READY_TIME_MS: u128 = u64::MAX as u128;

pub(crate) struct CandidateReadiness {
    path: Option<PathBuf>,
    invocation_id: String,
    attempted: bool,
}

impl CandidateReadiness {
    pub(crate) fn from_env() -> Self {
        let path = std::env::var_os(HEALTH_PATH_ENV)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        let invocation_id = std::env::var(INVOCATION_ID_ENV).unwrap_or_default();
        Self::new(path, invocation_id)
    }

    pub(crate) fn new(path: Option<PathBuf>, invocation_id: String) -> Self {
        Self {
            path,
            invocation_id,
            attempted: false,
        }
    }

    pub(crate) fn mark_ready(&mut self) {
        if self.attempted {
            return;
        }
        if self.path.is_none() {
            self.attempted = true;
            return;
        }
        let ready_at_unix_ms = match unix_time_ms() {
            Ok(value) => value,
            Err(error) => {
                self.log_failure_once(format!("clock unavailable: {error}"));
                return;
            }
        };
        self.mark_ready_at(std::process::id(), ready_at_unix_ms);
    }

    fn mark_ready_at(&mut self, pid: u32, ready_at_unix_ms: u64) {
        if self.attempted {
            return;
        }
        let Some(path) = self.path.as_deref() else {
            self.attempted = true;
            return;
        };
        let payload = CandidateHealthPayload {
            schema_version: 1,
            pid,
            systemd_invocation_id: self.invocation_id.clone(),
            package_version: env!("CARGO_PKG_VERSION").into(),
            board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
            ready_at_unix_ms,
        };
        let result = serde_json::to_vec_pretty(&payload)
            .map_err(|error| error.to_string())
            .and_then(|content| atomic_write(path, &content));
        self.attempted = true;
        if let Err(error) = result {
            eprintln!("candidate readiness marker unavailable: {path:?}: {error}");
        }
    }

    fn log_failure_once(&mut self, message: String) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        eprintln!("candidate readiness marker unavailable: {message}");
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct CandidateHealthPayload {
    schema_version: u8,
    pid: u32,
    systemd_invocation_id: String,
    package_version: String,
    board_profile: String,
    ready_at_unix_ms: u64,
}

fn unix_time_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(MAX_READY_TIME_MS) as u64)
        .map_err(|error| error.to_string())
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("candidate-ready.json");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = path.with_file_name(format!(".{name}.tmp-{}-{timestamp}", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o644);
        let mut file = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o644))
            .map_err(|error| error.to_string())?;
        file.write_all(content).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        replace_file(&temporary, path)?;
        if let Some(parent) = path.parent() {
            let _ = File::open(parent).and_then(|directory| directory.sync_all());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, path: &Path) -> Result<(), String> {
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn replace_file(temporary: &Path, path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let path = path
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let ok = unsafe { MoveFileExW(temporary.as_ptr(), path.as_ptr(), 0x1 | 0x8) };
    if ok == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::SystemTime;

    fn temporary_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "octessera-candidate-readiness-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn payload_uses_frozen_schema_and_field_names() {
        let payload = CandidateHealthPayload {
            schema_version: 1,
            pid: 1234,
            systemd_invocation_id: "invocation-1".into(),
            package_version: "0.7.0".into(),
            board_profile: "raspberry-pi-zero-2w".into(),
            ready_at_unix_ms: 1_700_000_000_123,
        };

        assert_eq!(
            serde_json::to_value(payload).unwrap(),
            serde_json::json!({
                "schema_version": 1,
                "pid": 1234,
                "systemd_invocation_id": "invocation-1",
                "package_version": "0.7.0",
                "board_profile": "raspberry-pi-zero-2w",
                "ready_at_unix_ms": 1_700_000_000_123_u64,
            })
        );
    }

    #[test]
    fn readiness_marker_is_atomic_replaced_and_mode_0644() {
        let directory = temporary_directory("atomic");
        let path = directory.join("candidate-ready.json");
        let mut readiness = CandidateReadiness::new(Some(path.clone()), "invocation-1".into());
        readiness.mark_ready_at(1234, 42);
        readiness.mark_ready_at(9999, 99);

        let payload: CandidateHealthPayload =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(payload.pid, 1234);
        assert_eq!(payload.ready_at_unix_ms, 42);
        assert!(std::fs::read_dir(&directory).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
        #[cfg(unix)]
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn missing_path_is_a_nonfatal_noop() {
        let mut readiness = CandidateReadiness::new(None, String::new());
        readiness.mark_ready();
        readiness.mark_ready();
        assert!(readiness.attempted);
    }
}
