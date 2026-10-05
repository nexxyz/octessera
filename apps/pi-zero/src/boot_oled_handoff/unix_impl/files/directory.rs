use super::super::super::*;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
#[cfg(test)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const ALLOWED_NAMES: [&str; 4] = [LOCK_NAME, STATUS_NAME, STOP_NAME, FATAL_NAME];

pub(in crate::boot_oled_handoff::unix_impl) struct RuntimeIdentity {
    pub(in crate::boot_oled_handoff::unix_impl) uid: u32,
    pub(in crate::boot_oled_handoff::unix_impl) gid: u32,
    pub(in crate::boot_oled_handoff::unix_impl) boot_id: String,
}

pub(in crate::boot_oled_handoff::unix_impl) struct HandoffDirectory {
    pub(in crate::boot_oled_handoff::unix_impl) file: File,
    pub(in crate::boot_oled_handoff::unix_impl) identity: RuntimeIdentity,
}

impl HandoffDirectory {
    #[cfg(test)]
    pub(in crate::boot_oled_handoff::unix_impl) fn open_runtime_at(
        path: &Path,
    ) -> Result<Self, String> {
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

    pub(in crate::boot_oled_handoff::unix_impl) fn open_existing_at(
        path: &Path,
    ) -> Result<Self, String> {
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

    pub(super) fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub(in crate::boot_oled_handoff::unix_impl) fn validate_entries(&self) -> Result<(), String> {
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
            if name == "." || name == ".." {
                continue;
            }
            if name == FATAL_NAME {
                let file = match open_named(self, &name, libc::O_RDONLY) {
                    Ok(file) => file,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        result = Err(format!("cannot open OLED fatal entry {name:?}: {error}"));
                        break;
                    }
                };
                if let Err(error) = validate_metadata(&file, true, FATAL_MODE, &self.identity) {
                    result = Err(error);
                    break;
                }
                continue;
            }
            if ALLOWED_NAMES.contains(&name.as_ref()) {
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

pub(super) fn atomic_temp_mode(name: &str) -> Option<u32> {
    for (prefix, mode) in [
        (".status.json.tmp-", STATUS_MODE),
        (".stop.request.tmp-", STOP_MODE),
        (".fatal.json.tmp-", FATAL_MODE),
    ] {
        if let Some(request_id) = name.strip_prefix(prefix) {
            return super::valid_request_id(request_id).then_some(mode);
        }
    }
    None
}

pub(super) fn validate_metadata(
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

pub(super) fn open_named(
    directory: &HandoffDirectory,
    name: &str,
    flags: i32,
) -> std::io::Result<File> {
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

pub(super) fn create_named(
    directory: &HandoffDirectory,
    name: &str,
    mode: u32,
) -> Result<File, String> {
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

pub(super) fn unlink_named(directory: &HandoffDirectory, name: &str) -> Result<(), String> {
    let name = CString::new(name).expect("handoff name");
    if unsafe { libc::unlinkat(directory.fd(), name.as_ptr(), 0) } != 0 {
        return Err(format!("cannot remove OLED handoff entry: {}", io_error()));
    }
    Ok(())
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(in crate::boot_oled_handoff::unix_impl) fn cleanup_temporary_files(
    directory: &HandoffDirectory,
) -> Result<(), String> {
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

pub(in crate::boot_oled_handoff::unix_impl) fn current_boot_id() -> Result<String, String> {
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|error| format!("cannot read kernel boot_id: {error}"))?
        .trim()
        .to_string();
    if !super::valid_boot_id(&boot_id) {
        return Err("kernel boot_id is malformed".into());
    }
    Ok(boot_id)
}

fn c_string(path: &Path) -> Result<CString, String> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| "OLED handoff path contains a NUL".into())
}

pub(super) fn io_error() -> std::io::Error {
    std::io::Error::last_os_error()
}
