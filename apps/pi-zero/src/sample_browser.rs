use playback_runtime::SampleEntry;
use std::path::Path;

const SD_CARD_SAMPLE_DIR: &str = "sd-card";
pub(crate) const SD_CARD_SAMPLE_BROWSER_DIR: &str = "sd-card/octessera/samples";

pub fn sample_entries(samples_dir: &Path, dir: &str) -> Result<Vec<SampleEntry>, String> {
    let dir = normalize_sample_dir(dir)?;
    if sd_card_path_requested(&dir) && !sd_card_samples_available(samples_dir) {
        return Err("SD card is not available. Insert the OLED SD card and try again.".into());
    }
    if std::fs::symlink_metadata(samples_dir)
        .map_err(|error| format!("sample root metadata failed: {error}"))?
        .file_type()
        .is_symlink()
    {
        return Err("sample root is symlinked".into());
    }
    let root = samples_dir
        .canonicalize()
        .map_err(|error| format!("sample root resolve failed: {error}"))?;
    let relative = if dir == "samples" {
        ""
    } else {
        dir.strip_prefix("samples/").unwrap_or(&dir)
    };
    reject_symlink_components(&root, relative)?;
    let requested_path = root.join(relative);
    let requested = match requested_path.canonicalize() {
        Ok(path) => path,
        Err(_error) if !requested_path.exists() => return Ok(Vec::new()),
        Err(error) => return Err(format!("sample directory resolve failed: {error}")),
    };
    if !requested.starts_with(&root) {
        return Err("sample directory outside sample root".into());
    }
    if !requested.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = sample_dir_entries(&root, &requested)?;
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

fn sample_dir_entries(root: &Path, requested: &Path) -> Result<Vec<SampleEntry>, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(requested).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
            return Err(format!(
                "sample tree contains a symlink: {}",
                entry.path().display()
            ));
        }
        let path = entry.path();
        let is_dir = path.is_dir();
        if !is_dir
            && path
                .extension()
                .is_none_or(|ext| !ext.to_string_lossy().eq_ignore_ascii_case("wav"))
        {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.contains('\\') {
            return Err(format!(
                "sample tree contains an invalid filename: {}",
                path.display()
            ));
        }
        entries.push(SampleEntry {
            name: name.to_string(),
            path: relative_path(root, &path, name, is_dir),
            is_dir,
        });
    }
    Ok(entries)
}

fn relative_path(root: &Path, path: &Path, fallback: &str, is_dir: bool) -> String {
    let relative = path
        .strip_prefix(root)
        .ok()
        .and_then(|path| path.to_str())
        .unwrap_or(fallback)
        .replace('\\', "/");
    if default_library_path(&relative, is_dir) {
        format!("samples/{relative}")
    } else {
        relative
    }
}

fn default_library_path(relative: &str, is_dir: bool) -> bool {
    if relative.is_empty() {
        return false;
    }
    include_str!("../../../samples/ATTRIBUTIONS.tsv")
        .lines()
        .skip(1)
        .filter_map(|line| line.split('\t').next())
        .filter(|path| path.to_ascii_lowercase().ends_with(".wav"))
        .any(|path| {
            if is_dir {
                path.starts_with(&format!("{relative}/"))
            } else {
                path == relative
            }
        })
}

fn normalize_sample_dir(input: &str) -> Result<String, String> {
    let input = input.trim().replace('\\', "/");
    if input.is_empty() {
        return Ok(String::new());
    }
    if input.starts_with('/')
        || input
            .split('/')
            .next()
            .is_some_and(|part| part.ends_with(':'))
    {
        return Err("absolute sample path is not allowed".into());
    }
    let mut parts = Vec::new();
    for part in input.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err("sample path traversal is not allowed".into());
        }
        parts.push(part);
    }
    Ok(parts.join("/"))
}

fn reject_symlink_components(root: &Path, relative: &str) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.split('/').filter(|part| !part.is_empty()) {
        current.push(component);
        if std::fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(format!(
                "sample tree contains a symlink: {}",
                current.display()
            ));
        }
    }
    Ok(())
}

fn sd_card_samples_available(samples_dir: &Path) -> bool {
    let path = samples_dir.join(SD_CARD_SAMPLE_DIR);
    path.is_dir() && is_mount_point(&path) && samples_dir.join(SD_CARD_SAMPLE_BROWSER_DIR).is_dir()
}

fn sd_card_path_requested(dir: &str) -> bool {
    dir == SD_CARD_SAMPLE_DIR || dir.starts_with("sd-card/")
}

fn is_mount_point(path: &Path) -> bool {
    let Ok(target) = path.canonicalize() else {
        return false;
    };
    std::fs::read_to_string("/proc/mounts")
        .map(|mounts| {
            mounts.lines().any(|line| {
                line.split_whitespace()
                    .nth(1)
                    .map(unescape_mount_path)
                    .is_some_and(|mount| Path::new(&mount) == target)
            })
        })
        .unwrap_or_else(|_| path.is_dir())
}

fn unescape_mount_path(path: &str) -> String {
    path.replace("\\040", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory {
        root: std::path::PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "octessera-pi-sample-browser-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn sample_entries_preserve_nonlibrary_local_ids_and_leave_parent_navigation_to_native_menu() {
        let directory = TestDirectory::new();
        let root = &directory.root;
        std::fs::create_dir_all(root.join("Drums").join("Loops")).unwrap();
        std::fs::write(root.join("Drums").join("hat.wav"), b"wav").unwrap();
        std::fs::write(root.join("Drums").join("ignore.txt"), b"text").unwrap();
        std::fs::write(root.join("kick.wav"), b"wav").unwrap();

        assert_eq!(
            sample_entries(root, "").unwrap(),
            vec![
                SampleEntry {
                    name: "Drums".into(),
                    path: "Drums".into(),
                    is_dir: true,
                },
                SampleEntry {
                    name: "kick.wav".into(),
                    path: "kick.wav".into(),
                    is_dir: false,
                },
            ]
        );
        assert_eq!(
            sample_entries(root, "Drums").unwrap(),
            vec![
                SampleEntry {
                    name: "Loops".into(),
                    path: "Drums/Loops".into(),
                    is_dir: true,
                },
                SampleEntry {
                    name: "hat.wav".into(),
                    path: "Drums/hat.wav".into(),
                    is_dir: false,
                },
            ]
        );
        assert!(sample_entries(root, "Drums/Loops").unwrap().is_empty());
    }

    #[test]
    fn sample_entries_normalize_legacy_builtin_dirs_at_browser_boundary() {
        let directory = TestDirectory::new();
        let root = &directory.root;
        std::fs::create_dir_all(root.join("Drum").join("kick")).unwrap();
        std::fs::write(root.join("Drum").join("kick").join("Kick2.wav"), b"wav").unwrap();

        assert_eq!(
            sample_entries(root, r"samples\Drum\kick").unwrap()[0].path,
            "samples/Drum/kick/Kick2.wav"
        );
        assert!(sample_entries(root, "../Drum").is_err());
    }

    #[test]
    fn repository_default_fixture_entries_are_canonical_sample_ids() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples");
        let entries = sample_entries(&root, "").expect("Pi default sample fixture");
        assert!(!entries.is_empty());
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.name == "Drum")
                .expect("Drum directory")
                .path,
            "samples/Drum"
        );
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.name == "upstream")
                .expect("metadata directory")
                .path,
            "upstream"
        );
        let entries = sample_entries(&root, "samples/Drum/kick").expect("kick fixture");
        assert!(entries
            .iter()
            .all(|entry| entry.path.starts_with("samples/Drum/kick/")));
        assert!(entries.iter().any(|entry| {
            entry.name == "Kick2.wav" && entry.path == "samples/Drum/kick/Kick2.wav"
        }));
    }
}
