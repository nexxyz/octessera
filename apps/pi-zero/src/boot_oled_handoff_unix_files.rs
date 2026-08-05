use super::super::*;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::thread;
use std::time::Instant;

const ALLOWED_NAMES: [&str; 3] = [LOCK_NAME, STATUS_NAME, STOP_NAME];

#[path = "boot_oled_handoff_unix_atomic.rs"]
mod atomic;
use atomic::atomic_write;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use atomic::{inject_atomic_failure, AtomicFailure};

pub(super) struct RuntimeIdentity {
    pub(super) uid: u32,
    pub(super) gid: u32,
    pub(super) boot_id: String,
}

pub(super) struct HandoffDirectory {
    pub(super) file: File,
    pub(super) identity: RuntimeIdentity,
}

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

impl HandoffDirectory {
    #[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
    pub(super) fn open_runtime_at(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            std::fs::create_dir(path)
                .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
            let mut permissions = std::fs::metadata(path)
                .map_err(|error| error.to_string())?
                .permissions();
            permissions.set_mode(DIRECTORY_MODE);
            std::fs::set_permissions(path, permissions).map_err(|error| error.to_string())?;
        }
        Self::open_at(path)
    }

    pub(super) fn open_existing_at(path: &Path) -> Result<Self, String> {
        Self::open_at(path)
    }

    fn open_at(path: &Path) -> Result<Self, String> {
        let path_c = c_string(path)?;
        let fd = unsafe {
            libc::open(
                path_c.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(format!(
                "cannot open OLED handoff directory: {}",
                io_error()
            ));
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let identity = RuntimeIdentity {
            uid: unsafe { libc::geteuid() },
            gid: unsafe { libc::getegid() },
            boot_id: current_boot_id()?,
        };
        validate_metadata(&file, false, DIRECTORY_MODE, &identity)?;
        Ok(Self { file, identity })
    }

    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub(super) fn validate_entries(&self) -> Result<(), String> {
        let duplicate = unsafe { libc::dup(self.fd()) };
        if duplicate < 0 {
            return Err(format!(
                "cannot duplicate OLED handoff directory fd: {}",
                io_error()
            ));
        }
        let dir = unsafe { libc::fdopendir(duplicate) };
        if dir.is_null() {
            unsafe { libc::close(duplicate) };
            return Err(format!(
                "cannot inspect OLED handoff directory: {}",
                io_error()
            ));
        }
        let mut result = Ok(());
        loop {
            let entry = unsafe { libc::readdir(dir) };
            if entry.is_null() {
                break;
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_string_lossy();
            if name == "." || name == ".." || ALLOWED_NAMES.contains(&name.as_ref()) {
                continue;
            }
            if let Some(mode) = atomic_temp_mode(&name) {
                let file = match open_named(self, &name, libc::O_RDONLY) {
                    Ok(file) => file,
                    Err(error) => {
                        result = Err(format!(
                            "cannot open OLED temporary entry {name:?}: {error}"
                        ));
                        break;
                    }
                };
                if let Err(error) = validate_metadata(&file, true, mode, &self.identity) {
                    result = Err(error);
                    break;
                }
                continue;
            }
            result = Err(format!("unknown OLED handoff entry {name:?}"));
            break;
        }
        unsafe { libc::closedir(dir) };
        result
    }
}

fn atomic_temp_mode(name: &str) -> Option<u32> {
    for (prefix, mode) in [
        (".status.json.tmp-", STATUS_MODE),
        (".stop.request.tmp-", STOP_MODE),
    ] {
        if let Some(request_id) = name.strip_prefix(prefix) {
            return valid_request_id(request_id).then_some(mode);
        }
    }
    None
}

fn validate_metadata(
    file: &File,
    regular: bool,
    mode: u32,
    identity: &RuntimeIdentity,
) -> Result<(), String> {
    let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
    if unsafe { libc::fstat(file.as_raw_fd(), &mut stat) } != 0 {
        return Err(format!("cannot stat OLED handoff entry: {}", io_error()));
    }
    let expected_type = if regular {
        libc::S_IFREG
    } else {
        libc::S_IFDIR
    };
    if stat.st_mode & libc::S_IFMT != expected_type {
        return Err("OLED handoff entry has the wrong file type".into());
    }
    if stat.st_uid != identity.uid || stat.st_gid != identity.gid {
        return Err("OLED handoff entry owner mismatch".into());
    }
    if stat.st_mode & 0o7777 != mode {
        return Err(format!(
            "OLED handoff entry mode mismatch: expected {mode:04o}"
        ));
    }
    if regular && stat.st_nlink != 1 {
        return Err("OLED handoff regular file must have link count one".into());
    }
    Ok(())
}

fn open_named(directory: &HandoffDirectory, name: &str, flags: i32) -> std::io::Result<File> {
    let name = CString::new(name).expect("fixed handoff name");
    let fd = unsafe {
        libc::openat(
            directory.fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn create_named(directory: &HandoffDirectory, name: &str, mode: u32) -> Result<File, String> {
    let name_c = CString::new(name).expect("handoff name");
    let fd = unsafe {
        libc::openat(
            directory.fd(),
            name_c.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            mode,
        )
    };
    if fd < 0 {
        return Err(format!(
            "cannot create OLED handoff entry {name}: {}",
            io_error()
        ));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    if unsafe { libc::fchmod(file.as_raw_fd(), mode) } != 0 {
        let error = io_error();
        drop(file);
        let _ = unsafe { libc::unlinkat(directory.fd(), name_c.as_ptr(), 0) };
        return Err(format!(
            "cannot set OLED handoff entry {name} mode: {error}"
        ));
    }
    Ok(file)
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
    let file = match open_named(directory, name, libc::O_RDONLY) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot open OLED handoff {name}: {error}")),
    };
    validate_metadata(&file, true, mode, &directory.identity)?;
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

#[path = "boot_oled_handoff_unix_contract.rs"]
mod contract;
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

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) fn cleanup_temporary_files(directory: &HandoffDirectory) -> Result<(), String> {
    let duplicate = unsafe { libc::dup(directory.fd()) };
    if duplicate < 0 {
        return Err(format!(
            "cannot duplicate OLED handoff directory fd: {}",
            io_error()
        ));
    }
    let dir = unsafe { libc::fdopendir(duplicate) };
    if dir.is_null() {
        unsafe { libc::close(duplicate) };
        return Err(format!(
            "cannot inspect OLED handoff directory: {}",
            io_error()
        ));
    }
    let mut names = Vec::new();
    loop {
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_string_lossy();
        if atomic_temp_mode(&name).is_some() {
            names.push(name.into_owned());
        }
    }
    unsafe { libc::closedir(dir) };
    for name in names {
        unlink_named(directory, &name)?;
    }
    Ok(())
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn unlink_named(directory: &HandoffDirectory, name: &str) -> Result<(), String> {
    let name = CString::new(name).expect("handoff name");
    if unsafe { libc::unlinkat(directory.fd(), name.as_ptr(), 0) } != 0 {
        return Err(format!("cannot remove OLED handoff entry: {}", io_error()));
    }
    Ok(())
}

pub(super) fn current_boot_id() -> Result<String, String> {
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|error| format!("cannot read kernel boot_id: {error}"))?
        .trim()
        .to_string();
    if !valid_boot_id(&boot_id) {
        return Err("kernel boot_id is malformed".into());
    }
    Ok(boot_id)
}

pub(super) fn random_request_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .map_err(|error| format!("cannot open /dev/urandom: {error}"))?
        .read_exact(&mut bytes)
        .map_err(|error| format!("cannot read /dev/urandom: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn c_string(path: &Path) -> Result<CString, String> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| "OLED handoff path contains a NUL".into())
}

fn io_error() -> std::io::Error {
    std::io::Error::last_os_error()
}
