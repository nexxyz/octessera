#[cfg(unix)]
use std::time::Duration;

pub(crate) const HANDOFF_ENV: &str = "OCTESSERA_OLED_BOOT_HANDOFF";
#[cfg(unix)]
pub(crate) const HANDOFF_ROOT: &str = "/run/octessera-boot";
#[cfg(unix)]
pub(crate) const HANDOFF_SCHEMA: u8 = 1;
#[cfg(unix)]
pub(crate) const NATIVE_LOCK_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(unix)]
const LOCK_NAME: &str = "oled.lock";
#[cfg(unix)]
const STATUS_NAME: &str = "status.json";
#[cfg(unix)]
const STOP_NAME: &str = "stop.request";
#[cfg(unix)]
const FATAL_NAME: &str = "fatal.json";
#[cfg(unix)]
const MAX_STATUS_BYTES: usize = 4096;
#[cfg(unix)]
const MAX_STOP_BYTES: usize = 1024;
#[cfg(all(unix, any(test, feature = "hardware-orange-pi-zero-2w")))]
const MAX_FATAL_BYTES: usize = 256;
#[cfg(unix)]
const DIRECTORY_MODE: u32 = 0o750;
#[cfg(unix)]
const LOCK_MODE: u32 = 0o600;
#[cfg(unix)]
const STATUS_MODE: u32 = 0o640;
#[cfg(unix)]
const STOP_MODE: u32 = 0o600;
#[cfg(unix)]
const FATAL_MODE: u32 = 0o600;

#[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
#[cfg_attr(not(unix), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupFatalCode {
    TrellisUnavailable,
    NeokeyUnavailable,
    ControlsUnavailable,
    AudioUnavailable,
    OledUnavailable,
    StartupFailed,
}

#[cfg(all(unix, any(test, feature = "hardware-orange-pi-zero-2w")))]
impl StartupFatalCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TrellisUnavailable => "trellis_unavailable",
            Self::NeokeyUnavailable => "neokey_unavailable",
            Self::ControlsUnavailable => "controls_unavailable",
            Self::AudioUnavailable => "audio_unavailable",
            Self::OledUnavailable => "oled_unavailable",
            Self::StartupFailed => "startup_failed",
        }
    }
}

#[cfg(all(unix, test))]
impl StartupFatalCode {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "trellis_unavailable" => Self::TrellisUnavailable,
            "neokey_unavailable" => Self::NeokeyUnavailable,
            "controls_unavailable" => Self::ControlsUnavailable,
            "audio_unavailable" => Self::AudioUnavailable,
            "oled_unavailable" => Self::OledUnavailable,
            "startup_failed" => Self::StartupFailed,
            _ => return None,
        })
    }
}

#[cfg(all(unix, any(test, feature = "hardware-orange-pi-zero-2w")))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StartupFatal {
    pub(crate) boot_id: String,
    pub(crate) code: StartupFatalCode,
}

#[cfg(all(unix, any(test, feature = "hardware-orange-pi-zero-2w")))]
impl StartupFatal {
    fn new(boot_id: String, code: StartupFatalCode) -> Self {
        Self { boot_id, code }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": HANDOFF_SCHEMA,
            "bootId": self.boot_id,
            "code": self.code.as_str(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HandoffMode {
    Legacy,
    V1,
}

pub(crate) fn mode_from_env() -> Result<HandoffMode, String> {
    match std::env::var(HANDOFF_ENV) {
        Ok(value) => parse_mode_value(Some(&value)),
        Err(std::env::VarError::NotPresent) => parse_mode_value(None),
        Err(error) => Err(format!("{HANDOFF_ENV} is unavailable: {error}")),
    }
}

fn parse_mode_value(value: Option<&str>) -> Result<HandoffMode, String> {
    match value {
        Some("v1") => Ok(HandoffMode::V1),
        Some(value) => Err(format!(
            "{HANDOFF_ENV} has unsupported value {value:?}; expected v1"
        )),
        None => Ok(HandoffMode::Legacy),
    }
}

#[cfg(any(unix, all(test, not(feature = "hardware-orange-pi-zero-2w"))))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HandoffPhase {
    Animating,
    ReleaseRequested,
    Released,
    NativeOwned,
    FirstMenuRendered,
    Failed,
}

#[cfg(any(unix, all(test, not(feature = "hardware-orange-pi-zero-2w"))))]
impl HandoffPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Animating => "animating",
            Self::ReleaseRequested => "release_requested",
            Self::Released => "released",
            Self::NativeOwned => "native_owned",
            Self::FirstMenuRendered => "first_menu_rendered",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "animating" => Self::Animating,
            "release_requested" => Self::ReleaseRequested,
            "released" => Self::Released,
            "native_owned" => Self::NativeOwned,
            "first_menu_rendered" => Self::FirstMenuRendered,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[cfg(unix)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HandoffStatus {
    pub(crate) phase: HandoffPhase,
    pub(crate) boot_id: String,
    pub(crate) pid: u32,
    pub(crate) cycle_count: u64,
    pub(crate) request_id: Option<String>,
}

#[cfg(unix)]
impl HandoffStatus {
    fn new(
        phase: HandoffPhase,
        boot_id: String,
        cycle_count: u64,
        request_id: Option<String>,
    ) -> Self {
        Self {
            phase,
            boot_id,
            pid: std::process::id(),
            cycle_count,
            request_id,
        }
    }

    fn json(&self) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert("schema".into(), serde_json::json!(HANDOFF_SCHEMA));
        object.insert("phase".into(), serde_json::json!(self.phase.as_str()));
        object.insert("bootId".into(), serde_json::json!(self.boot_id));
        object.insert("pid".into(), serde_json::json!(self.pid));
        object.insert("cycleCount".into(), serde_json::json!(self.cycle_count));
        if let Some(request_id) = &self.request_id {
            object.insert("requestId".into(), serde_json::json!(request_id));
        }
        serde_json::Value::Object(object)
    }
}

#[cfg(unix)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct StopRequest {
    boot_id: String,
    pid: u32,
    request_id: String,
}

#[cfg(unix)]
impl StopRequest {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": HANDOFF_SCHEMA,
            "bootId": self.boot_id,
            "pid": self.pid,
            "requestId": self.request_id,
        })
    }
}

#[cfg(unix)]
#[path = "boot_oled_handoff_unix.rs"]
mod unix_impl;

#[cfg(all(unix, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) use unix_impl::native_attach_after_startup_clear;
#[cfg(all(test, unix))]
pub(crate) use unix_impl::native_guard_for_test;
#[cfg(all(unix, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) use unix_impl::publish_startup_fatal;
#[cfg(all(unix, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use unix_impl::AnimatorHandoff;
#[cfg(all(unix, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) use unix_impl::NativeOledGuard;
#[cfg(all(unix, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use unix_impl::{animator_start, utility_lock};
#[cfg(all(unix, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use unix_impl::{native_attach, NativeOledGuard};

#[cfg(all(not(unix), not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) struct AnimatorHandoff;

#[cfg(not(unix))]
pub(crate) struct NativeOledGuard;

#[cfg(all(not(unix), not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) struct UtilityOledLock;

#[cfg(all(not(unix), not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn animator_start() -> Result<AnimatorHandoff, String> {
    Err("OLED boot handoff requires Unix file locking".into())
}

#[cfg(all(not(unix), not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn native_attach() -> Result<NativeOledGuard, String> {
    Err("OLED boot handoff requires Unix file locking".into())
}

#[cfg(all(not(unix), feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn publish_startup_fatal(_code: StartupFatalCode) -> Result<(), String> {
    Err("OLED boot handoff requires Unix file locking".into())
}

#[cfg(all(not(unix), feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn native_attach_after_startup_clear() -> Result<NativeOledGuard, String> {
    Err("OLED boot handoff requires Unix file locking".into())
}

#[cfg(all(not(unix), not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn utility_lock() -> Result<UtilityOledLock, String> {
    Err("OLED boot handoff requires Unix file locking".into())
}

#[cfg(all(not(unix), not(feature = "hardware-orange-pi-zero-2w")))]
impl AnimatorHandoff {
    pub(crate) fn stop_requested(&mut self) -> Result<bool, String> {
        Err("OLED boot handoff requires Unix file locking".into())
    }
    pub(crate) fn publish_cycle(&mut self) -> Result<(), String> {
        Err("OLED boot handoff requires Unix file locking".into())
    }
    pub(crate) fn mark_failed(&mut self) {}
    pub(crate) fn release(self) -> Result<(), String> {
        Err("OLED boot handoff requires Unix file locking".into())
    }
}

#[cfg(not(unix))]
impl NativeOledGuard {
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn mark_unavailable_and_failed(
        &self,
        _code: StartupFatalCode,
    ) -> Result<(), String> {
        Err("OLED boot handoff requires Unix file locking".into())
    }

    pub(crate) fn detach_preserving(&mut self) -> Result<(), String> {
        Err("OLED boot handoff requires Unix file locking".into())
    }
    pub(crate) fn reacquire_existing(&mut self) -> Result<(), String> {
        Err("OLED boot handoff requires Unix file locking".into())
    }
    pub(crate) fn mark_first_menu_rendered(&mut self) -> Result<(), String> {
        Err("OLED boot handoff requires Unix file locking".into())
    }
    pub(crate) fn mark_failed_result(&self) -> Result<(), String> {
        Err("OLED boot handoff requires Unix file locking".into())
    }
    #[allow(dead_code)]
    pub(crate) fn mark_failed(&self) {}
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
mod tests {
    use super::*;

    #[test]
    fn phase_names_are_frozen() {
        assert_eq!(HandoffPhase::Animating.as_str(), "animating");
        assert_eq!(HandoffPhase::ReleaseRequested.as_str(), "release_requested");
        assert_eq!(HandoffPhase::Released.as_str(), "released");
        assert_eq!(HandoffPhase::NativeOwned.as_str(), "native_owned");
        assert_eq!(
            HandoffPhase::FirstMenuRendered.as_str(),
            "first_menu_rendered"
        );
        assert_eq!(HandoffPhase::Failed.as_str(), "failed");
    }

    #[test]
    fn unknown_phase_is_rejected() {
        assert_eq!(HandoffPhase::parse("unknown"), None);
    }

    #[test]
    fn handoff_mode_missing_is_legacy_and_invalid_is_closed() {
        assert_eq!(parse_mode_value(None), Ok(HandoffMode::Legacy));
        assert_eq!(parse_mode_value(Some("v1")), Ok(HandoffMode::V1));
        assert!(parse_mode_value(Some("v2")).is_err());
    }
}
