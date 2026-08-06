use crate::setup_portal_files::SetupPortalPaths;
use playback_runtime::{RuntimeSetupPortalStatus, RuntimeStoreResult};
use serde::Deserialize;
use std::fs;
use std::sync::Arc;
use std::time::Instant;

pub(crate) const SCHEMA: u64 = 1;
pub(crate) const MAX_SEQUENCE: u64 = i64::MAX as u64;

pub(crate) type ClockSource = Arc<dyn Fn() -> u64 + Send + Sync>;
pub(crate) type RandomSource = Arc<dyn Fn(&mut [u8]) -> Result<(), String> + Send + Sync>;
pub(crate) type BootIdSource = Arc<dyn Fn() -> Result<String, String> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct SetupPortalEnvironment {
    pub(crate) paths: SetupPortalPaths,
    pub(crate) status_group: Result<u32, String>,
    pub(crate) expected_owner_uid: u32,
    pub(crate) clock: ClockSource,
    pub(crate) random: RandomSource,
    pub(crate) boot_id: BootIdSource,
}

impl SetupPortalEnvironment {
    pub(crate) fn production() -> Self {
        let paths = SetupPortalPaths::production();
        let boot_id_path = paths.boot_id.clone();
        Self {
            paths,
            status_group: public_group_gid(),
            expected_owner_uid: 0,
            clock: Arc::new(system_monotonic_millis),
            random: Arc::new(fill_cryptographic_random),
            boot_id: Arc::new(move || read_boot_id_path(&boot_id_path)),
        }
    }

    #[cfg(test)]
    pub(crate) fn test(
        paths: SetupPortalPaths,
        status_group: u32,
        clock: ClockSource,
        random: RandomSource,
        boot_id: BootIdSource,
    ) -> Self {
        Self {
            paths,
            status_group: Ok(status_group),
            expected_owner_uid: test_owner_uid(),
            clock,
            random,
            boot_id,
        }
    }
}

#[cfg(all(test, unix))]
fn test_owner_uid() -> u32 {
    unsafe { libc::geteuid() }
}

#[cfg(all(test, not(unix)))]
fn test_owner_uid() -> u32 {
    0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusEnvelope {
    schema: u64,
    #[serde(rename = "bootId")]
    boot_id: String,
    #[serde(rename = "attemptId")]
    attempt_id: String,
    sequence: u64,
    status: RuntimeStoreResult,
}

pub(crate) struct ValidatedStatusEnvelope {
    pub(crate) boot_id: String,
    pub(crate) attempt_id: String,
    pub(crate) sequence: u64,
    pub(crate) status: RuntimeSetupPortalStatus,
}

impl<'de> Deserialize<'de> for ValidatedStatusEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = StatusEnvelope::deserialize(deserializer)?;
        if raw.schema != SCHEMA
            || !valid_boot_id(&raw.boot_id)
            || !crate::setup_portal_files::valid_hex_32(&raw.attempt_id)
            || raw.sequence == 0
            || raw.sequence > MAX_SEQUENCE
        {
            return Err(serde::de::Error::custom("invalid setup portal envelope"));
        }
        let RuntimeStoreResult::SetupPortalStatus { status } = raw.status else {
            return Err(serde::de::Error::custom(
                "invalid setup portal status envelope",
            ));
        };
        status.validate().map_err(serde::de::Error::custom)?;
        Ok(Self {
            boot_id: raw.boot_id,
            attempt_id: raw.attempt_id,
            sequence: raw.sequence,
            status,
        })
    }
}

pub(crate) fn make_request_token(random: &RandomSource) -> Result<String, String> {
    let mut bytes = [0u8; 16];
    random(&mut bytes)?;
    let mut token = String::with_capacity(32);
    for byte in bytes {
        token.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        token.push(char::from(b"0123456789abcdef"[(byte & 0x0f) as usize]));
    }
    Ok(token)
}

pub(crate) fn valid_boot_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) && byte == b'-'
                || !matches!(index, 8 | 13 | 18 | 23)
                    && byte.is_ascii_hexdigit()
                    && !byte.is_ascii_uppercase()
        })
}

fn system_monotonic_millis() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn read_boot_id_path(path: &std::path::Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|value| value.trim().to_string())
        .map_err(|_| "kernel boot identity is unavailable".into())
}

fn public_group_gid() -> Result<u32, String> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let name = if cfg!(feature = "hardware-orange-pi-zero-2w") {
            "octessera-runtime"
        } else {
            "pi"
        };
        let name = CString::new(name).map_err(|_| "setup portal group is invalid".to_string())?;
        let group = unsafe { libc::getgrnam(name.as_ptr()) };
        if group.is_null() {
            return Err("setup portal group is unavailable".into());
        }
        Ok(unsafe { (*group).gr_gid })
    }
    #[cfg(not(unix))]
    {
        Err("setup portal group is unavailable".into())
    }
}

fn fill_cryptographic_random(bytes: &mut [u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let path =
            CString::new("/dev/urandom").map_err(|_| "random source is unavailable".to_string())?;
        let descriptor = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        if descriptor < 0 {
            return Err("random source is unavailable".into());
        }
        let mut offset = 0;
        while offset < bytes.len() {
            let read = unsafe {
                libc::read(
                    descriptor,
                    bytes[offset..].as_mut_ptr().cast(),
                    (bytes.len() - offset) as libc::size_t,
                )
            };
            if read <= 0 {
                unsafe { libc::close(descriptor) };
                return Err("random source is unavailable".into());
            }
            offset += read as usize;
        }
        unsafe { libc::close(descriptor) };
        Ok(())
    }
    #[cfg(windows)]
    {
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                0x00000002,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err("random source is unavailable".into())
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = bytes;
        Err("random source is unavailable".into())
    }
}

#[cfg(windows)]
#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(
        algorithm: *mut std::ffi::c_void,
        buffer: *mut u8,
        length: u32,
        flags: u32,
    ) -> i32;
}
