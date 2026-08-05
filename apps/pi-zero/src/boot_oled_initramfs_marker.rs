use super::files::{current_boot_id, valid_boot_id};
use super::*;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub(super) fn validate_initramfs_marker_if_present() -> Result<bool, String> {
    let Some(file) = open_marker(Path::new(INITRAMFS_MARKER_PATH))? else {
        return Ok(false);
    };
    validate_marker_file(file)?;
    Ok(true)
}

fn open_marker(path: &Path) -> Result<Option<File>, String> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| "initramfs OLED marker path contains a NUL".to_string())?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        let error = io_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(format!("cannot open initramfs OLED marker: {error}"));
    }
    Ok(Some(unsafe { File::from_raw_fd(fd) }))
}

fn validate_marker_file(file: File) -> Result<(), String> {
    let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
    if unsafe { libc::fstat(file.as_raw_fd(), &mut stat) } != 0 {
        return Err(format!("cannot stat initramfs OLED marker: {}", io_error()));
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG
        || stat.st_uid != 0
        || stat.st_gid != 0
        || stat.st_mode & 0o7777 != 0o644
        || stat.st_nlink != 1
    {
        return Err(
            "initramfs OLED marker has invalid ownership, mode, type, or link count".into(),
        );
    }
    if stat.st_size < 0 || stat.st_size > 256 {
        return Err("initramfs OLED marker has invalid size".into());
    }
    let mut bytes = Vec::new();
    (&file)
        .take(257)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read initramfs OLED marker: {error}"))?;
    if bytes.len() > 256 {
        return Err("initramfs OLED marker exceeds 256 bytes".into());
    }
    parse_marker(&bytes, &current_boot_id()?)
}

fn parse_marker(bytes: &[u8], boot_id: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("malformed initramfs OLED marker: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "initramfs OLED marker must be an object".to_string())?;
    if object.len() != 2
        || !object
            .keys()
            .all(|key| ["schema", "bootId"].contains(&key.as_str()))
    {
        return Err("initramfs OLED marker has unknown or missing keys".into());
    }
    if object.get("schema").and_then(|value| value.as_u64()) != Some(1)
        || object
            .get("bootId")
            .and_then(|value| value.as_str())
            .filter(|value| valid_boot_id(value))
            != Some(boot_id)
    {
        return Err("initramfs OLED marker does not identify the current boot".into());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn parse_marker_for_test(bytes: &[u8], boot_id: &str) -> Result<(), String> {
    parse_marker(bytes, boot_id)
}

#[cfg(test)]
pub(crate) fn validate_marker_at_for_test(path: &Path) -> Result<(), String> {
    let file = open_marker(path)?.ok_or_else(|| "initramfs OLED marker is missing".to_string())?;
    validate_marker_file(file)
}

#[cfg(test)]
pub(crate) fn validate_marker_if_present_at_for_test(path: &Path) -> Result<bool, String> {
    let Some(file) = open_marker(path)? else {
        return Ok(false);
    };
    validate_marker_file(file)?;
    Ok(true)
}

fn io_error() -> std::io::Error {
    std::io::Error::last_os_error()
}
