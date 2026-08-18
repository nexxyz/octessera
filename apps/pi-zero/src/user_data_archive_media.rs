use super::MediaFile;
use playback_runtime::{UserDataMediaKind, UserDataMediaReference, USER_DATA_MAX_MEDIA_BYTES};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

pub(super) fn sample_media(samples_dir: &Path) -> Result<Vec<MediaFile>, String> {
    let samples_metadata = match fs::symlink_metadata(samples_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(error)),
    };
    if samples_metadata.file_type().is_symlink() {
        return Err("sample directory is a symlink".into());
    }
    if !samples_metadata.is_dir() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    collect_sample_files(samples_dir, samples_dir, &mut result)?;
    Ok(result)
}

fn collect_sample_files(
    root: &Path,
    current: &Path,
    result: &mut Vec<MediaFile>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            return Err("sample directory contains a symlink".into());
        }
        if metadata.is_dir() {
            collect_sample_files(root, &path, result)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "sample path escaped its root".to_string())?;
        let relative = relative
            .to_str()
            .ok_or_else(|| "sample file name is not valid UTF-8".to_string())?
            .replace('\\', "/");
        if is_packaged_sample(&relative) {
            continue;
        }
        if relative.contains('/') {
            return Err("nested custom sample directories are not supported yet".into());
        }
        if !playback_runtime::is_safe_user_data_name(&relative) {
            return Err("custom sample file name is not safe for transfer".into());
        }
        let reference = media_reference_for_file(&relative, &path, UserDataMediaKind::Sample)?;
        result.push(MediaFile { reference, path });
    }
    Ok(())
}

pub(super) fn directory_media(
    directory: &Path,
    kind: UserDataMediaKind,
    label: &str,
) -> Result<Vec<MediaFile>, String> {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(error)),
    };
    if metadata.file_type().is_symlink() {
        return Err(format!("{label} directory is a symlink"));
    }
    if !metadata.is_dir() {
        return Err(format!("{label} directory is not a directory"));
    }
    let mut media = Vec::new();
    for entry in fs::read_dir(directory).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{label} directory contains a symlink"));
        }
        if metadata.is_dir() {
            return Err(format!("nested {label} directories are not supported yet"));
        }
        if !metadata.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{label} file name is not valid UTF-8"))?;
        if !playback_runtime::is_safe_user_data_name(name) {
            return Err(format!("{label} file name is not safe for transfer"));
        }
        media.push(MediaFile {
            reference: media_reference_for_file(name, &path, kind.clone())?,
            path,
        });
    }
    Ok(media)
}

fn media_reference_for_file(
    name: &str,
    path: &Path,
    kind: UserDataMediaKind,
) -> Result<UserDataMediaReference, String> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        size = size
            .checked_add(read as u64)
            .ok_or_else(|| "custom sample size overflowed".to_string())?;
        if size > USER_DATA_MAX_MEDIA_BYTES {
            return Err("custom sample exceeds its size limit".into());
        }
        hasher.update(&buffer[..read]);
    }
    let bytes = hasher.finalize();
    let sha256 = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(UserDataMediaReference {
        id: name.into(),
        kind,
        display_name: name.into(),
        size,
        sha256,
    })
}

fn is_packaged_sample(name: &str) -> bool {
    include_str!("../../../samples/ATTRIBUTIONS.tsv")
        .lines()
        .skip(1)
        .filter_map(|line| line.split('\t').next())
        .any(|path| path == name)
}

pub(super) fn kind_name(kind: &UserDataMediaKind) -> &'static str {
    match kind {
        UserDataMediaKind::Sample => "sample",
        UserDataMediaKind::Audio => "audio",
        UserDataMediaKind::Screen => "screen",
    }
}

pub(super) fn media_key(reference: &UserDataMediaReference) -> (&'static str, &str) {
    (kind_name(&reference.kind), reference.id.as_str())
}

pub(super) fn media_kind_byte(kind: &UserDataMediaKind) -> u8 {
    match kind {
        UserDataMediaKind::Sample => 0,
        UserDataMediaKind::Audio => 1,
        UserDataMediaKind::Screen => 2,
    }
}

pub(super) fn media_kind_from_byte(value: u8) -> Result<UserDataMediaKind, String> {
    match value {
        0 => Ok(UserDataMediaKind::Sample),
        1 => Ok(UserDataMediaKind::Audio),
        2 => Ok(UserDataMediaKind::Screen),
        _ => Err("user-data archive media kind is invalid".into()),
    }
}

pub(super) fn media_stage_dir(kind: &UserDataMediaKind) -> &'static str {
    match kind {
        UserDataMediaKind::Sample => "samples",
        UserDataMediaKind::Audio => "audio",
        UserDataMediaKind::Screen => "screen",
    }
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}
