#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

mod request_marker;

pub(crate) use crate::setup_portal_paths::SetupPortalPaths;
pub(crate) use request_marker::create_request_marker;
#[cfg(test)]
pub(crate) use request_marker::create_request_marker_with_publisher_for_test;

pub(crate) const MAX_STATUS_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetupFileKind {
    Regular,
    Directory,
    Symlink,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SetupMetadata {
    pub(crate) kind: SetupFileKind,
    pub(crate) uid: Option<u32>,
    pub(crate) gid: Option<u32>,
    pub(crate) mode: u32,
    pub(crate) nlink: u64,
    pub(crate) size: u64,
}

impl SetupMetadata {
    #[cfg(test)]
    pub(crate) fn directory(uid: u32, gid: u32, mode: u32) -> Self {
        Self {
            kind: SetupFileKind::Directory,
            uid: Some(uid),
            gid: Some(gid),
            mode,
            nlink: 1,
            size: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn regular(uid: u32, gid: u32, mode: u32, size: u64) -> Self {
        Self {
            kind: SetupFileKind::Regular,
            uid: Some(uid),
            gid: Some(gid),
            mode,
            nlink: 1,
            size,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetupFileError {
    Missing,
    Exists,
    Permission,
    Unsafe,
    Oversized,
    Io,
    Published,
}

pub(crate) fn read_status_file(
    paths: &SetupPortalPaths,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<Option<Vec<u8>>, SetupFileError> {
    validate_directory_path(
        &paths.public,
        Some(expected_uid),
        Some((expected_gid, 0o750)),
    )?;
    match read_bounded_file(&paths.current, expected_uid, expected_gid, 0o640) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(SetupFileError::Missing) => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn validate_directory_path(
    path: &Path,
    expected_uid: Option<u32>,
    expected_group_mode: Option<(u32, u32)>,
) -> Result<(), SetupFileError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => return Err(classify_io(error)),
    };
    let metadata = metadata_from_std(&metadata);
    if metadata.kind != SetupFileKind::Directory {
        return Err(SetupFileError::Unsafe);
    }
    if let Some(uid) = expected_uid {
        if metadata.uid != Some(uid) {
            return Err(SetupFileError::Unsafe);
        }
    }
    if let Some((gid, mode)) = expected_group_mode {
        if metadata.gid != Some(gid) || metadata.mode != mode {
            return Err(SetupFileError::Unsafe);
        }
    }
    Ok(())
}

pub(crate) fn validate_public_metadata(
    metadata: SetupMetadata,
    expected_uid: u32,
    expected_gid: u32,
    mode: u32,
    directory: bool,
) -> Result<(), SetupFileError> {
    let expected_kind = if directory {
        SetupFileKind::Directory
    } else {
        SetupFileKind::Regular
    };
    if metadata.kind != expected_kind
        || metadata.uid != Some(expected_uid)
        || metadata.gid != Some(expected_gid)
        || metadata.mode != mode
        || metadata.nlink != 1
    {
        return Err(SetupFileError::Unsafe);
    }
    Ok(())
}

fn read_bounded_file(
    path: &Path,
    expected_uid: u32,
    expected_gid: u32,
    expected_mode: u32,
) -> Result<Vec<u8>, SetupFileError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => return Err(classify_io(error)),
    };
    let metadata = metadata_from_std(&metadata);
    validate_public_metadata(metadata, expected_uid, expected_gid, expected_mode, false)?;
    if metadata.size > MAX_STATUS_BYTES {
        return Err(SetupFileError::Oversized);
    }
    let mut file = open_existing_file(path)?;
    let opened_metadata = file_metadata(&file).map_err(classify_io)?;
    validate_public_metadata(
        opened_metadata,
        expected_uid,
        expected_gid,
        expected_mode,
        false,
    )?;
    if opened_metadata.size > MAX_STATUS_BYTES {
        return Err(SetupFileError::Oversized);
    }
    let mut bytes =
        Vec::with_capacity((opened_metadata.size as usize).min(MAX_STATUS_BYTES as usize));
    let mut chunk = [0u8; 4096];
    loop {
        let read = file.read(&mut chunk).map_err(classify_io)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() as u64 > MAX_STATUS_BYTES {
            return Err(SetupFileError::Oversized);
        }
    }
    Ok(bytes)
}

fn open_existing_file(path: &Path) -> Result<File, SetupFileError> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::fd::FromRawFd;
        let path = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| SetupFileError::Unsafe)?;
        let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
        let descriptor = unsafe { libc::open(path.as_ptr(), flags) };
        if descriptor < 0 {
            return Err(classify_errno());
        }
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
    #[cfg(not(unix))]
    {
        OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(classify_io)
    }
}

fn metadata_from_std(metadata: &fs::Metadata) -> SetupMetadata {
    let kind = if metadata.file_type().is_symlink() {
        SetupFileKind::Symlink
    } else if metadata.is_dir() {
        SetupFileKind::Directory
    } else if metadata.is_file() {
        SetupFileKind::Regular
    } else {
        SetupFileKind::Other
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        SetupMetadata {
            kind,
            uid: Some(metadata.uid()),
            gid: Some(metadata.gid()),
            mode: metadata.mode() & 0o7777,
            nlink: metadata.nlink(),
            size: metadata.size(),
        }
    }
    #[cfg(not(unix))]
    {
        #[cfg(test)]
        {
            SetupMetadata {
                kind,
                uid: Some(0),
                gid: Some(0),
                mode: if kind == SetupFileKind::Directory {
                    0o750
                } else if metadata.len() == 6 {
                    0o600
                } else {
                    0o640
                },
                nlink: 1,
                size: metadata.len(),
            }
        }
        #[cfg(not(test))]
        SetupMetadata {
            kind,
            uid: None,
            gid: None,
            mode: if metadata.permissions().readonly() {
                0o444
            } else {
                0o666
            },
            nlink: 1,
            size: metadata.len(),
        }
    }
}

fn file_metadata(file: &File) -> std::io::Result<SetupMetadata> {
    file.metadata().map(|metadata| metadata_from_std(&metadata))
}

fn classify_io(error: std::io::Error) -> SetupFileError {
    match error.kind() {
        std::io::ErrorKind::NotFound => SetupFileError::Missing,
        std::io::ErrorKind::PermissionDenied => SetupFileError::Permission,
        std::io::ErrorKind::AlreadyExists => SetupFileError::Exists,
        _ => SetupFileError::Io,
    }
}

#[cfg(unix)]
fn classify_errno() -> SetupFileError {
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ELOOP) {
        SetupFileError::Unsafe
    } else {
        classify_io(error)
    }
}
