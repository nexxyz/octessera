use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Why a restore cannot place one entry of a live folder.
#[derive(Debug)]
pub(crate) enum PlacementError {
    Io(io::Error),
    /// A folder restore may not write, so it can neither move nor replace it.
    Unmovable(PathBuf),
}

impl fmt::Display for PlacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Unmovable(path) => write!(
                formatter,
                "restore cannot move {}: Octessera may not write that folder",
                path.display()
            ),
        }
    }
}

impl From<io::Error> for PlacementError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<PlacementError> for String {
    fn from(error: PlacementError) -> Self {
        error.to_string()
    }
}

/// What restore may touch. It never copies, moves or deletes across a mount (the SD card inside
/// the samples folder). It leaves the bundled library folders in place when it may not write them
/// (the Orange's root-owned library); any other folder it may not write fails the restore. Tests
/// swap in a fake that marks chosen folders.
pub(crate) trait Filesystems {
    fn device(&self, path: &Path) -> io::Result<u64>;

    fn is_writable_dir(&self, path: &Path) -> io::Result<bool>;

    /// The lock every SD-card mount and transfer takes; restore holds it while it works.
    fn sd_card_lock_path(&self) -> &Path {
        boot_sd_card_lock_path()
    }

    fn is_other_filesystem(&self, root: &Path, path: &Path) -> io::Result<bool> {
        Ok(self.device(root)? != self.device(path)?)
    }

    /// Whether restore leaves this entry of `root` where it is, neither copied nor moved.
    /// `holds_library` says `root` is the samples folder.
    fn stays_in_place(
        &self,
        root: &Path,
        path: &Path,
        holds_library: bool,
    ) -> Result<bool, PlacementError> {
        if self.is_other_filesystem(root, path)? {
            return Ok(true);
        }
        if !fs::symlink_metadata(path)?.is_dir() || self.is_writable_dir(path)? {
            return Ok(false);
        }
        if holds_library && is_bundled_library_folder(root, path) {
            return Ok(true);
        }
        Err(PlacementError::Unmovable(path.to_path_buf()))
    }
}

#[cfg(not(test))]
fn boot_sd_card_lock_path() -> &'static Path {
    Path::new("/run/octessera-sd-card.lock")
}

/// Tests share one lock file in place of the one the boards create at boot.
#[cfg(test)]
fn boot_sd_card_lock_path() -> &'static Path {
    static LOCK: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(|| {
        let path = std::env::temp_dir().join("octessera-test-sd-card.lock");
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        path
    });
    &LOCK
}

fn is_bundled_library_folder(root: &Path, path: &Path) -> bool {
    path.parent() == Some(root)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(crate::sample_browser::is_packaged_library_folder)
}

pub(crate) struct LiveFilesystems;

impl Filesystems for LiveFilesystems {
    #[cfg(unix)]
    fn device(&self, path: &Path) -> io::Result<u64> {
        use std::os::unix::fs::MetadataExt;
        Ok(fs::symlink_metadata(path)?.dev())
    }

    #[cfg(not(unix))]
    fn device(&self, path: &Path) -> io::Result<u64> {
        fs::symlink_metadata(path).map(|_| 0)
    }

    #[cfg(unix)]
    fn is_writable_dir(&self, path: &Path) -> io::Result<bool> {
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::other("path contains a NUL byte"))?;
        if unsafe { libc::access(path.as_ptr(), libc::W_OK) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EACCES | libc::EROFS) => Ok(false),
            _ => Err(error),
        }
    }

    #[cfg(not(unix))]
    fn is_writable_dir(&self, path: &Path) -> io::Result<bool> {
        Ok(!fs::symlink_metadata(path)?.permissions().readonly())
    }
}

/// Deletes `path` and everything under it, refusing to descend into another filesystem.
pub(crate) fn remove_tree_within(
    filesystems: &dyn Filesystems,
    root: &Path,
    path: &Path,
) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry_path = entry?.path();
        if filesystems.is_other_filesystem(root, &entry_path)? {
            return Err(io::Error::other(format!(
                "refusing to delete across a mount: {}",
                entry_path.display()
            )));
        }
        let metadata = fs::symlink_metadata(&entry_path)?;
        if metadata.is_dir() {
            remove_tree_within(filesystems, root, &entry_path)?;
        } else {
            fs::remove_file(&entry_path)?;
        }
    }
    fs::remove_dir(path)
}
