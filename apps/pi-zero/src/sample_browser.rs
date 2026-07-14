use playback_runtime::SampleEntry;
use std::path::Path;

const SD_CARD_SAMPLE_DIR: &str = "sd-card";
pub(crate) const SD_CARD_SAMPLE_BROWSER_DIR: &str = "sd-card/octessera/samples";

pub fn sample_entries(samples_dir: &Path, dir: &str) -> Result<Vec<SampleEntry>, String> {
    if sd_card_path_requested(dir) && !sd_card_samples_available(samples_dir) {
        return Err("SD card is not available. Insert the OLED SD card and try again.".into());
    }
    let root = samples_dir
        .canonicalize()
        .unwrap_or_else(|_| samples_dir.to_path_buf());
    let requested = root
        .join(dir)
        .canonicalize()
        .unwrap_or_else(|_| root.join(dir));
    if !requested.starts_with(&root) {
        return Err("sample directory outside sample root".into());
    }
    if !requested.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = parent_entry(&root, &requested)
        .into_iter()
        .collect::<Vec<_>>();
    entries.extend(sample_dir_entries(&root, &requested)?);
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

fn parent_entry(root: &Path, requested: &Path) -> Option<SampleEntry> {
    (requested != root).then(|| SampleEntry {
        name: "..".into(),
        path: parent_relative(root, requested),
        is_dir: true,
    })
}

fn sample_dir_entries(root: &Path, requested: &Path) -> Result<Vec<SampleEntry>, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(requested).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let is_dir = path.is_dir();
        if !is_dir && path.extension().is_none_or(|ext| ext != "wav") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        entries.push(SampleEntry {
            name: name.to_string(),
            path: relative_path(root, &path, name),
            is_dir,
        });
    }
    Ok(entries)
}

fn relative_path(root: &Path, path: &Path, fallback: &str) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(|path| path.to_str())
        .unwrap_or(fallback)
        .replace('\\', "/")
}

fn parent_relative(root: &Path, requested: &Path) -> String {
    requested
        .parent()
        .unwrap_or(root)
        .strip_prefix(root)
        .ok()
        .and_then(|path| path.to_str())
        .unwrap_or("")
        .replace('\\', "/")
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
