#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

#[cfg(unix)]
use super::classify_errno;
use super::{
    classify_io, file_metadata, validate_directory_path, SetupFileError, SetupFileKind,
    SetupMetadata,
};

pub(crate) fn create_request_marker(path: &Path) -> Result<(), SetupFileError> {
    create_request_marker_with_publisher(path, &publish_marker_noreplace)
}

#[cfg(test)]
pub(crate) fn create_request_marker_with_publisher_for_test<F>(
    path: &Path,
    publisher: F,
) -> Result<(), SetupFileError>
where
    F: Fn(&Path, &Path) -> Result<(), SetupFileError>,
{
    create_request_marker_with_publisher(path, &publisher)
}

fn create_request_marker_with_publisher(
    path: &Path,
    publisher: &dyn Fn(&Path, &Path) -> Result<(), SetupFileError>,
) -> Result<(), SetupFileError> {
    let Some(parent) = path.parent() else {
        return Err(SetupFileError::Unsafe);
    };
    validate_request_parent(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Err(SetupFileError::Unsafe),
        Ok(_) => return Err(SetupFileError::Exists),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(classify_io(error))
        }
        Err(_) => {}
    }
    let temp_path = path.with_file_name(".start.tmp");
    let content = b"start\n";
    let mut owns_temp = false;
    let result = (|| {
        let mut file = open_new_file(&temp_path, 0o600)?;
        owns_temp = true;
        set_file_mode(&file, 0o600)?;
        file.write_all(content).map_err(classify_io)?;
        file.sync_all().map_err(classify_io)?;
        validate_marker_content(&mut file, content)?;
        let metadata = file_metadata(&file).map_err(classify_io)?;
        if !valid_marker_metadata(metadata) {
            return Err(SetupFileError::Unsafe);
        }
        drop(file);
        publisher(&temp_path, path)?;
        owns_temp = false;
        sync_directory(parent).map_err(|_| SetupFileError::Published)
    })();
    if result.is_err() && owns_temp {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn validate_request_parent(path: &Path) -> Result<(), SetupFileError> {
    #[cfg(unix)]
    {
        validate_directory_path(
            path,
            Some(unsafe { libc::geteuid() }),
            Some((unsafe { libc::getegid() }, 0o700)),
        )
    }
    #[cfg(not(unix))]
    {
        validate_directory_path(path, None, None)
    }
}

fn open_new_file(path: &Path, mode: u32) -> Result<File, SetupFileError> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::fd::FromRawFd;
        let path = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| SetupFileError::Unsafe)?;
        let flags =
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW;
        let descriptor = unsafe { libc::open(path.as_ptr(), flags, mode as libc::mode_t) };
        if descriptor < 0 {
            return Err(classify_errno());
        }
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
    #[cfg(not(unix))]
    {
        let _ = mode;
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(classify_io)
    }
}

fn set_file_mode(file: &File, mode: u32) -> Result<(), SetupFileError> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let result = unsafe { libc::fchmod(file.as_raw_fd(), mode as libc::mode_t) };
        if result != 0 {
            return Err(classify_errno());
        }
    }
    #[cfg(not(unix))]
    {
        let _ = mode;
        let mut permissions = file.metadata().map_err(classify_io)?.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        file.set_permissions(permissions).map_err(classify_io)?;
    }
    Ok(())
}

fn valid_marker_metadata(metadata: SetupMetadata) -> bool {
    metadata.kind == SetupFileKind::Regular
        && marker_owner_matches(metadata)
        && metadata.mode == 0o600
        && metadata.nlink == 1
        && metadata.size == 6
}

fn marker_owner_matches(metadata: SetupMetadata) -> bool {
    #[cfg(unix)]
    {
        metadata.uid == Some(unsafe { libc::geteuid() })
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        true
    }
}

fn validate_marker_content(file: &mut File, expected: &[u8]) -> Result<(), SetupFileError> {
    file.seek(SeekFrom::Start(0)).map_err(classify_io)?;
    let mut actual = vec![0; expected.len()];
    file.read_exact(&mut actual).map_err(classify_io)?;
    if actual != expected {
        return Err(SetupFileError::Unsafe);
    }
    let mut extra = [0; 1];
    if file.read(&mut extra).map_err(classify_io)? != 0 {
        return Err(SetupFileError::Unsafe);
    }
    Ok(())
}

fn publish_marker_noreplace(source: &Path, destination: &Path) -> Result<(), SetupFileError> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        let source = CString::new(source.as_os_str().as_encoded_bytes())
            .map_err(|_| SetupFileError::Unsafe)?;
        let destination = CString::new(destination.as_os_str().as_encoded_bytes())
            .map_err(|_| SetupFileError::Unsafe)?;
        let result = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                1_u32,
            )
        };
        if result != 0 {
            return Err(classify_errno());
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        fs::rename(source, destination).map_err(classify_io)
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let _ = (source, destination);
        Err(SetupFileError::Io)
    }
}

fn sync_directory(path: &Path) -> Result<(), SetupFileError> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::fd::FromRawFd;
        let path = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| SetupFileError::Unsafe)?;
        let descriptor = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if descriptor < 0 {
            return Err(classify_errno());
        }
        let directory = unsafe { File::from_raw_fd(descriptor) };
        directory.sync_all().map_err(classify_io)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        Ok(())
    }
}
