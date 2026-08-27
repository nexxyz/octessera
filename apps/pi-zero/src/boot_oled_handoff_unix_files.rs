use super::super::*;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::thread;
use std::time::Instant;

#[path = "boot_oled_handoff_unix_atomic.rs"]
mod atomic;
use atomic::atomic_write;
#[cfg(test)]
pub(crate) use atomic::{inject_atomic_failure, AtomicFailure};

#[path = "boot_oled_handoff_unix_directory.rs"]
mod directory;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) use directory::cleanup_temporary_files;
#[cfg(test)]
pub(super) use directory::current_boot_id;
pub(super) use directory::HandoffDirectory;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(super) use directory::RuntimeIdentity;
use directory::{create_named, io_error, open_named, unlink_named, validate_metadata};

pub(super) fn open_lock(directory: &HandoffDirectory, create: bool) -> Result<File, String> {
    match open_named(directory, LOCK_NAME, libc::O_RDWR) {
        Ok(file) => {
            validate_metadata(&file, true, LOCK_MODE, &directory.identity)?;
            Ok(file)
        }
        Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
            let file = create_named(directory, LOCK_NAME, LOCK_MODE)?;
            validate_metadata(&file, true, LOCK_MODE, &directory.identity)?;
            Ok(file)
        }
        Err(error) => Err(format!("cannot open OLED lock: {error}")),
    }
}

pub(super) fn flock(file: &File, nonblocking: bool) -> Result<(), String> {
    let operation = libc::LOCK_EX | if nonblocking { libc::LOCK_NB } else { 0 };
    if unsafe { libc::flock(file.as_raw_fd(), operation) } == 0 {
        return Ok(());
    }
    let error = io_error();
    if nonblocking
        && error
            .raw_os_error()
            .is_some_and(|code| code == libc::EWOULDBLOCK || code == libc::EAGAIN)
    {
        return Err(format!("temporarily unavailable: {error}"));
    }
    Err(error.to_string())
}

pub(super) fn acquire_native_lock(lock: &File) -> Result<(), String> {
    let deadline = Instant::now() + NATIVE_LOCK_TIMEOUT;
    loop {
        match flock(lock, true) {
            Ok(()) => return Ok(()),
            Err(error) if Instant::now() < deadline && error.starts_with("temporarily") => {
                thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(error) => {
                return Err(format!(
                    "OLED handoff lock was not released within 2s: {error}"
                ))
            }
        }
    }
}

pub(super) fn create_or_attach_stop(
    directory: &HandoffDirectory,
    status: &HandoffStatus,
) -> Result<String, String> {
    let request_id = if let Some(request_id) = &status.request_id {
        request_id.clone()
    } else if let Some(existing) = read_stop(directory)? {
        existing.request_id
    } else {
        random_request_id()?
    };
    match write_stop(
        directory,
        &StopRequest {
            boot_id: directory.identity.boot_id.clone(),
            pid: std::process::id(),
            request_id: request_id.clone(),
        },
    )? {
        true => Ok(request_id),
        false => {
            let Some(existing) = read_stop(directory)? else {
                return Err("OLED stop request disappeared after no-clobber publish".into());
            };
            if existing.boot_id != directory.identity.boot_id
                || (status.request_id.is_some() && existing.request_id != request_id)
            {
                return Err("OLED stop request is already claimed".into());
            }
            Ok(existing.request_id)
        }
    }
}

pub(super) fn write_status(
    directory: &HandoffDirectory,
    status: &HandoffStatus,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(&status.json()).map_err(|error| error.to_string())?;
    atomic_write(
        directory,
        STATUS_NAME,
        STATUS_MODE,
        &bytes,
        MAX_STATUS_BYTES,
        false,
    )
    .map(|_| ())
}

#[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn write_fatal(
    directory: &HandoffDirectory,
    fatal: &StartupFatal,
) -> Result<(), String> {
    let _ = open_validated_named(directory, FATAL_NAME, FATAL_MODE)?;
    let bytes = serde_json::to_vec(&fatal.json()).map_err(|error| error.to_string())?;
    atomic_write(
        directory,
        FATAL_NAME,
        FATAL_MODE,
        &bytes,
        MAX_FATAL_BYTES,
        false,
    )
    .map(|_| ())
}

#[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn clear_fatal(directory: &HandoffDirectory) -> Result<(), String> {
    let Some(_file) = open_validated_named(directory, FATAL_NAME, FATAL_MODE)? else {
        return Ok(());
    };
    unlink_named(directory, FATAL_NAME)?;
    directory
        .file
        .sync_all()
        .map_err(|error| format!("cannot sync OLED handoff directory: {error}"))
}

#[cfg(test)]
pub(super) fn read_fatal(directory: &HandoffDirectory) -> Result<Option<StartupFatal>, String> {
    let Some(bytes) = read_named(directory, FATAL_NAME, FATAL_MODE, MAX_FATAL_BYTES)? else {
        return Ok(None);
    };
    let fatal = parse_fatal(&bytes)?;
    if fatal.boot_id != directory.identity.boot_id {
        return Err("OLED fatal.json belongs to a different boot".into());
    }
    Ok(Some(fatal))
}

fn write_stop(directory: &HandoffDirectory, request: &StopRequest) -> Result<bool, String> {
    let bytes = serde_json::to_vec(&request.json()).map_err(|error| error.to_string())?;
    atomic_write(
        directory,
        STOP_NAME,
        STOP_MODE,
        &bytes,
        MAX_STOP_BYTES,
        true,
    )
}

pub(super) fn read_status(directory: &HandoffDirectory) -> Result<Option<HandoffStatus>, String> {
    let Some(bytes) = read_named(directory, STATUS_NAME, STATUS_MODE, MAX_STATUS_BYTES)? else {
        return Ok(None);
    };
    parse_status(&bytes).map(Some)
}

pub(super) fn read_stop(directory: &HandoffDirectory) -> Result<Option<StopRequest>, String> {
    let Some(bytes) = read_named(directory, STOP_NAME, STOP_MODE, MAX_STOP_BYTES)? else {
        return Ok(None);
    };
    parse_stop(&bytes).map(Some)
}

fn read_named(
    directory: &HandoffDirectory,
    name: &str,
    mode: u32,
    max: usize,
) -> Result<Option<Vec<u8>>, String> {
    let Some(file) = open_validated_named(directory, name, mode)? else {
        return Ok(None);
    };
    if file.metadata().map_err(|error| error.to_string())?.len() > max as u64 {
        return Err(format!("OLED handoff {name} exceeds {max} bytes"));
    }
    let mut bytes = Vec::new();
    (&file)
        .take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > max {
        return Err(format!("OLED handoff {name} exceeds {max} bytes"));
    }
    Ok(Some(bytes))
}

fn open_validated_named(
    directory: &HandoffDirectory,
    name: &str,
    mode: u32,
) -> Result<Option<File>, String> {
    let file = match open_named(directory, name, libc::O_RDONLY) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot open OLED handoff {name}: {error}")),
    };
    validate_metadata(&file, true, mode, &directory.identity)?;
    Ok(Some(file))
}

#[path = "boot_oled_handoff_unix_contract.rs"]
mod contract;
#[cfg(test)]
pub(super) use contract::parse_fatal;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use contract::parse_status_for_test;
pub(super) use contract::{parse_status, parse_stop, valid_boot_id, valid_request_id};

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn cleanup_previous_state(directory: &HandoffDirectory) -> Result<(), String> {
    for (name, mode, max) in [
        (STATUS_NAME, STATUS_MODE, MAX_STATUS_BYTES),
        (STOP_NAME, STOP_MODE, MAX_STOP_BYTES),
    ] {
        let Some(bytes) = read_named(directory, name, mode, max)? else {
            continue;
        };
        if name == STATUS_NAME {
            let _ = parse_status(&bytes)?;
        } else {
            let _ = parse_stop(&bytes)?;
        }
        unlink_named(directory, name)?;
    }
    Ok(())
}

pub(super) fn random_request_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .map_err(|error| format!("cannot open /dev/urandom: {error}"))?
        .read_exact(&mut bytes)
        .map_err(|error| format!("cannot read /dev/urandom: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
