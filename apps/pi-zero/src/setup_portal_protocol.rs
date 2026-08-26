use crate::setup_portal_files::SetupPortalPaths;
use playback_runtime::{RuntimeSetupPortalPhase, RuntimeSetupPortalStatus, RuntimeStoreResult};
use serde::Deserialize;

pub(crate) const SCHEMA: u64 = 1;

#[derive(Clone)]
pub(crate) struct SetupPortalEnvironment {
    pub(crate) paths: SetupPortalPaths,
    pub(crate) status_group: Result<u32, String>,
    pub(crate) expected_owner_uid: u32,
}

impl SetupPortalEnvironment {
    pub(crate) fn production() -> Self {
        Self {
            paths: SetupPortalPaths::production(),
            status_group: public_group_gid(),
            expected_owner_uid: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn test(paths: SetupPortalPaths, status_group: u32) -> Self {
        Self {
            paths,
            status_group: Ok(status_group),
            expected_owner_uid: test_owner_uid(),
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
    status: RuntimeStoreResult,
}

pub(crate) struct ValidatedStatusEnvelope {
    pub(crate) status: RuntimeSetupPortalStatus,
}

impl<'de> Deserialize<'de> for ValidatedStatusEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = StatusEnvelope::deserialize(deserializer)?;
        if raw.schema != SCHEMA {
            return Err(serde::de::Error::custom("invalid setup portal envelope"));
        }
        let RuntimeStoreResult::SetupPortalStatus { status } = raw.status else {
            return Err(serde::de::Error::custom(
                "invalid setup portal status envelope",
            ));
        };
        status.validate().map_err(serde::de::Error::custom)?;
        if status.phase == RuntimeSetupPortalPhase::Unsupported {
            return Err(serde::de::Error::custom(
                "unsupported setup portal status is not an image phase",
            ));
        }
        Ok(Self { status })
    }
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
