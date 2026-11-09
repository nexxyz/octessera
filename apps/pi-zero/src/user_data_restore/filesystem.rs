use std::fs;
use std::io;
use std::path::Path;

/// What restore may touch. It never copies, moves or deletes across a mount (the SD card inside
/// the samples folder), and it cannot move a directory it may not write (the Orange's root-owned
/// bundled library). Tests swap in a fake that marks chosen folders.
pub(crate) trait Filesystems {
    fn device(&self, path: &Path) -> io::Result<u64>;

    fn is_writable_dir(&self, path: &Path) -> io::Result<bool>;

    fn is_other_filesystem(&self, root: &Path, path: &Path) -> io::Result<bool> {
        Ok(self.device(root)? != self.device(path)?)
    }

    /// Entries of `root` that a restore leaves where they are, neither copied nor moved.
    fn stays_in_place(&self, root: &Path, path: &Path) -> io::Result<bool> {
        if self.is_other_filesystem(root, path)? {
            return Ok(true);
        }
        let metadata = fs::symlink_metadata(path)?;
        Ok(metadata.is_dir() && !self.is_writable_dir(path)?)
    }
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
        Ok(unsafe { libc::access(path.as_ptr(), libc::W_OK) } == 0)
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
