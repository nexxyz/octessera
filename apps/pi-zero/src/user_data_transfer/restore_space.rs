use crate::user_data_restore::Filesystems;
use std::fs;
use std::path::Path;

pub(super) fn validate_free_space(
    filesystems: &dyn Filesystems,
    store_dir: &Path,
    samples_dir: &Path,
    recordings_dir: &Path,
    screen_recordings_dir: &Path,
    incoming: u64,
) -> Result<(), String> {
    let existing = [
        store_dir,
        samples_dir,
        recordings_dir,
        screen_recordings_dir,
    ]
    .into_iter()
    .map(|dir| directory_size(filesystems, dir, dir))
    .fold(0, u64::saturating_add);
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

fn directory_size(filesystems: &dyn Filesystems, root: &Path, path: &Path) -> u64 {
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
        .map(|entry| entry.path())
        .filter(|entry| matches!(filesystems.is_other_filesystem(root, entry), Ok(false)))
        .map(|entry| directory_size(filesystems, root, &entry))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::path::PathBuf;

    struct MountedAt(PathBuf);

    impl Filesystems for MountedAt {
        fn device(&self, path: &Path) -> io::Result<u64> {
            fs::symlink_metadata(path)?;
            Ok(u64::from(path.starts_with(&self.0)))
        }

        fn is_writable_dir(&self, _: &Path) -> io::Result<bool> {
            Ok(true)
        }
    }

    #[test]
    fn directory_size_skips_the_sd_card_mounted_inside_samples() {
        let samples =
            std::env::temp_dir().join(format!("octessera-restore-space-{}", std::process::id()));
        let card = samples.join("SD");
        fs::create_dir_all(card.join("Kits")).unwrap();
        fs::write(samples.join("live.wav"), [0; 5]).unwrap();
        fs::write(card.join("card.wav"), [0; 100]).unwrap();
        fs::write(card.join("Kits").join("kick.wav"), [0; 1000]).unwrap();

        let size = directory_size(&MountedAt(card), &samples, &samples);

        assert_eq!(size, 5);
        let _ = fs::remove_dir_all(samples);
    }
}
