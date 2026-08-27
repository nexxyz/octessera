use super::*;
use std::fs::File;
use std::path::Path;

#[path = "boot_oled_handoff_unix_files.rs"]
mod files;
use files::*;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) struct AnimatorHandoff {
    directory: HandoffDirectory,
    lock: File,
    boot_id: String,
    cycle_count: u64,
    request_id: Option<String>,
}

struct NativeOledLease(File);

impl NativeOledLease {
    fn fd(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        self.0.as_raw_fd()
    }
}

pub(crate) struct NativeOledGuard {
    directory: HandoffDirectory,
    lease: Option<NativeOledLease>,
    boot_id: String,
    cycle_count: u64,
    request_id: String,
    initial_menu_acknowledged: bool,
    first_menu_rendered: bool,
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) struct UtilityOledLock {
    _directory: HandoffDirectory,
    _lock: File,
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn utility_lock() -> Result<UtilityOledLock, String> {
    let directory = HandoffDirectory::open_existing_at(Path::new(HANDOFF_ROOT))?;
    directory.validate_entries()?;
    let lock = open_lock(&directory, false)?;
    flock(&lock, true)?;
    Ok(UtilityOledLock {
        _directory: directory,
        _lock: lock,
    })
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn animator_start() -> Result<AnimatorHandoff, String> {
    animator_start_at(Path::new(HANDOFF_ROOT))
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn animator_start_at(path: &Path) -> Result<AnimatorHandoff, String> {
    let directory = HandoffDirectory::open_existing_at(path)?;
    let lock = open_lock(&directory, true)?;
    flock(&lock, true)?;
    directory.validate_entries()?;
    match read_status(&directory)? {
        Some(status) if status.boot_id == directory.identity.boot_id => {
            return Err("OLED handoff already exists for this boot; refusing to clobber it".into())
        }
        Some(status) => {
            if let Some(stop) = read_stop(&directory)? {
                if stop.boot_id == directory.identity.boot_id {
                    return Err("OLED stop request already exists for this boot".into());
                }
                if stop.boot_id != status.boot_id {
                    return Err("OLED stale handoff entries belong to different boots".into());
                }
            }
            cleanup_previous_state(&directory)?;
        }
        None => {
            if let Some(stop) = read_stop(&directory)? {
                if stop.boot_id == directory.identity.boot_id {
                    return Err("OLED stop request already exists for this boot".into());
                }
                cleanup_previous_state(&directory)?;
            }
        }
    }
    cleanup_temporary_files(&directory)?;
    let boot_id = directory.identity.boot_id.clone();
    write_status(
        &directory,
        &HandoffStatus::new(HandoffPhase::Animating, boot_id.clone(), 0, None),
    )?;
    Ok(AnimatorHandoff {
        directory,
        lock,
        boot_id,
        cycle_count: 0,
        request_id: None,
    })
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
impl AnimatorHandoff {
    pub(crate) fn stop_requested(&mut self) -> Result<bool, String> {
        let Some(request) = read_stop(&self.directory)? else {
            return Ok(false);
        };
        if request.boot_id != self.boot_id {
            return Err("OLED stop request belongs to a different boot".into());
        }
        if let Some(existing) = &self.request_id {
            if existing != &request.request_id {
                return Err("OLED stop request changed during animation".into());
            }
        } else {
            self.request_id = Some(request.request_id.clone());
        }
        write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::ReleaseRequested,
                self.boot_id.clone(),
                self.cycle_count,
                self.request_id.clone(),
            ),
        )?;
        Ok(true)
    }

    pub(crate) fn publish_cycle(&mut self) -> Result<(), String> {
        if self.request_id.is_some() {
            return Err("OLED animation cannot publish a cycle after release was requested".into());
        }
        self.cycle_count = self.cycle_count.saturating_add(1);
        write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::Animating,
                self.boot_id.clone(),
                self.cycle_count,
                None,
            ),
        )
    }

    pub(crate) fn mark_failed(&mut self) {
        let status = HandoffStatus::new(
            HandoffPhase::Animating,
            self.boot_id.clone(),
            self.cycle_count,
            self.request_id.clone(),
        );
        let Ok(request_id) = create_or_attach_stop(&self.directory, &status) else {
            return;
        };
        self.request_id = Some(request_id.clone());
        let _ = write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::Failed,
                self.boot_id.clone(),
                self.cycle_count,
                Some(request_id),
            ),
        );
    }

    pub(crate) fn release(mut self) -> Result<(), String> {
        if self.request_id.is_none() {
            return Err("OLED animation release requires a validated stop request".into());
        }
        let result = write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::Released,
                self.boot_id.clone(),
                self.cycle_count,
                self.request_id.clone(),
            ),
        );
        if result.is_err() {
            self.mark_failed();
            return result;
        }
        let _ = self.lock.sync_all();
        Ok(())
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn native_attach() -> Result<NativeOledGuard, String> {
    native_attach_at(Path::new(HANDOFF_ROOT))
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn publish_startup_fatal(code: StartupFatalCode) -> Result<(), String> {
    publish_fatal_at(Path::new(HANDOFF_ROOT), code)
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn native_attach_after_startup_clear() -> Result<NativeOledGuard, String> {
    native_attach_after_startup_clear_at(Path::new(HANDOFF_ROOT))
}

#[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
fn native_attach_after_startup_clear_at(path: &Path) -> Result<NativeOledGuard, String> {
    let guard = native_attach_at(path)?;
    guard.clear_fatal()?;
    Ok(guard)
}

#[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
fn publish_fatal_at(path: &Path, code: StartupFatalCode) -> Result<(), String> {
    let directory = HandoffDirectory::open_existing_at(path)?;
    directory.validate_entries()?;
    write_fatal(
        &directory,
        &StartupFatal::new(directory.identity.boot_id.clone(), code),
    )
}

fn native_attach_at(path: &Path) -> Result<NativeOledGuard, String> {
    let directory = HandoffDirectory::open_existing_at(path)?;
    directory.validate_entries()?;
    let request = match read_status(&directory)? {
        Some(status) => match status.phase {
            HandoffPhase::Animating | HandoffPhase::ReleaseRequested => {
                create_or_attach_stop(&directory, &status)?
            }
            HandoffPhase::Released
            | HandoffPhase::NativeOwned
            | HandoffPhase::FirstMenuRendered => status
                .request_id
                .ok_or_else(|| "OLED status has no requestId".to_string())?,
            HandoffPhase::Failed => status
                .request_id
                .ok_or_else(|| "OLED failed status has no requestId".to_string())?,
        },
        None => return Err("OLED boot handoff status is missing".into()),
    };
    let lock = open_lock(&directory, false)?;
    acquire_native_lock(&lock)?;
    let status = read_status(&directory)?.ok_or_else(|| "OLED status disappeared".to_string())?;
    if status.boot_id != directory.identity.boot_id {
        return Err("OLED status belongs to a different boot".into());
    }
    if !matches!(
        status.phase,
        HandoffPhase::Released
            | HandoffPhase::NativeOwned
            | HandoffPhase::FirstMenuRendered
            | HandoffPhase::Failed
    ) {
        return Err(format!(
            "OLED handoff stopped in phase {}",
            status.phase.as_str()
        ));
    }
    if status.request_id.as_deref() != Some(request.as_str()) {
        return Err("OLED status requestId does not match stop request".into());
    }
    let stop = read_stop(&directory)?.ok_or_else(|| "OLED stop request is missing".to_string())?;
    if stop.boot_id != status.boot_id || stop.request_id != request {
        return Err("OLED stop request does not match status".into());
    }
    let boot_id = directory.identity.boot_id.clone();
    let cycle_count = status.cycle_count;
    write_status(
        &directory,
        &HandoffStatus::new(
            HandoffPhase::NativeOwned,
            boot_id.clone(),
            cycle_count,
            Some(request.clone()),
        ),
    )?;
    Ok(NativeOledGuard {
        directory,
        lease: Some(NativeOledLease(lock)),
        boot_id,
        cycle_count,
        request_id: request,
        initial_menu_acknowledged: false,
        first_menu_rendered: false,
    })
}

#[cfg(all(
    test,
    not(any(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "hardware-orange-pi-zero-2w"
    ))
))]
pub(crate) fn native_guard_for_test(path: &Path) -> Result<NativeOledGuard, String> {
    let directory = HandoffDirectory::open_runtime_at(path)?;
    let status = HandoffStatus::new(
        HandoffPhase::Released,
        directory.identity.boot_id.clone(),
        7,
        Some("0123456789abcdef0123456789abcdef".into()),
    );
    write_status(&directory, &status)?;
    create_or_attach_stop(&directory, &status)?;
    open_lock(&directory, true)?;
    native_attach_at(path)
}

impl NativeOledGuard {
    pub(crate) fn detach_preserving(&mut self) -> Result<(), String> {
        if self.lease.take().is_some() {
            Ok(())
        } else {
            Err("OLED native handoff lease is already released".into())
        }
    }

    pub(crate) fn reacquire_existing(&mut self) -> Result<(), String> {
        if self.lease.is_some() {
            return Err("OLED native handoff lease is already held".into());
        }
        let lock = open_lock(&self.directory, false)?;
        acquire_native_lock(&lock)?;
        let status =
            read_status(&self.directory)?.ok_or_else(|| "OLED status disappeared".to_string())?;
        if status.boot_id != self.boot_id
            || status.request_id.as_deref() != Some(self.request_id.as_str())
            || !matches!(
                status.phase,
                HandoffPhase::NativeOwned | HandoffPhase::FirstMenuRendered
            )
        {
            return Err("OLED native handoff status changed during reacquire".into());
        }
        let stop = read_stop(&self.directory)?
            .ok_or_else(|| "OLED stop request disappeared".to_string())?;
        if stop.boot_id != self.boot_id || stop.request_id != self.request_id {
            return Err("OLED stop request changed during reacquire".into());
        }
        self.lease = Some(NativeOledLease(lock));
        Ok(())
    }

    pub(crate) fn mark_first_menu_rendered(&mut self) -> Result<(), String> {
        let _ = self
            .lease
            .as_ref()
            .ok_or_else(|| "OLED native handoff lease is not held".to_string())?
            .fd();
        self.initial_menu_acknowledged = true;
        let result = write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::FirstMenuRendered,
                self.boot_id.clone(),
                self.cycle_count,
                Some(self.request_id.clone()),
            ),
        );
        if result.is_ok() {
            self.first_menu_rendered = true;
        }
        result
    }

    #[cfg_attr(feature = "hardware-orange-pi-zero-2w", allow(dead_code))]
    pub(crate) fn mark_failed(&self) {
        if let Err(error) = self.mark_failed_result() {
            eprintln!("OLED handoff failure-state publication failed: {error}");
        }
    }

    pub(crate) fn mark_failed_result(&self) -> Result<(), String> {
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        {
            let code = if self.initial_menu_acknowledged && !self.first_menu_rendered {
                StartupFatalCode::StartupFailed
            } else {
                StartupFatalCode::OledUnavailable
            };
            self.publish_fatal_then_failed(code)
        }
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        {
            self.write_failed_status()
        }
    }

    #[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
    fn clear_fatal(&self) -> Result<(), String> {
        let _ = self
            .lease
            .as_ref()
            .ok_or_else(|| "OLED native handoff lease is not held".to_string())?
            .fd();
        clear_fatal(&self.directory)
    }

    #[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn mark_unavailable_and_failed(&self, code: StartupFatalCode) -> Result<(), String> {
        self.publish_fatal_then_failed(code)
    }

    fn write_failed_status(&self) -> Result<(), String> {
        let _ = self
            .lease
            .as_ref()
            .ok_or_else(|| "OLED native handoff lease is not held".to_string())?
            .fd();
        write_status(
            &self.directory,
            &HandoffStatus::new(
                HandoffPhase::Failed,
                self.boot_id.clone(),
                self.cycle_count,
                Some(self.request_id.clone()),
            ),
        )
    }

    #[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
    fn publish_fatal_then_failed(&self, code: StartupFatalCode) -> Result<(), String> {
        let _ = self
            .lease
            .as_ref()
            .ok_or_else(|| "OLED native handoff lease is not held".to_string())?
            .fd();
        let fatal_result = write_fatal(
            &self.directory,
            &StartupFatal::new(self.boot_id.clone(), code),
        );
        let status_result = self.write_failed_status();
        match (fatal_result, status_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(fatal_error), Ok(())) => {
                Err(format!("OLED fatal publication failed: {fatal_error}"))
            }
            (Ok(()), Err(status_error)) => {
                Err(format!("OLED failed status publication failed: {status_error}"))
            }
            (Err(fatal_error), Err(status_error)) => Err(format!(
                "OLED fatal publication failed: {fatal_error}; OLED failed status publication failed: {status_error}"
            )),
        }
    }
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
#[path = "boot_oled_handoff_unix_tests.rs"]
mod tests;

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
#[path = "boot_oled_handoff_orange_tests.rs"]
mod orange_tests;
