use std::fs;
use std::path::Path;

pub(super) fn validate_free_space(
    store_dir: &Path,
    samples_dir: &Path,
    recordings_dir: &Path,
    screen_recordings_dir: &Path,
    incoming: u64,
) -> Result<(), String> {
    let existing = directory_size(store_dir)
        .saturating_add(directory_size(samples_dir))
        .saturating_add(directory_size(recordings_dir))
        .saturating_add(directory_size(screen_recordings_dir));
    let required = existing
        .saturating_add(incoming)
        .saturating_add(4 * 1024 * 1024);
    let Some(available) = available_space(store_dir) else {
        return Ok(());
    };
    if available < required {
        return Err("insufficient space for restore".into());
    }
    Ok(())
}

fn directory_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return metadata.len();
    }
    if !metadata.is_dir() {
        return 0;
    }
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| directory_size(&entry.path()))
        .sum()
}

#[cfg(unix)]
fn available_space(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    let path = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    let stats = unsafe { stats.assume_init() };
    Some(stats.f_bavail.saturating_mul(stats.f_frsize))
}

#[cfg(not(unix))]
fn available_space(_path: &Path) -> Option<u64> {
    None
}
