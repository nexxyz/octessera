use std::path::{Path, PathBuf};

pub(crate) fn valid_preset_name(name: &str) -> bool {
    playback_runtime::is_valid_preset_name(name)
}

pub(crate) fn preset_patch_file_path(presets_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(presets_dir.join("patches").join(format!("{name}.json")))
}

pub(crate) fn preset_name_from_file_name(file_name: &str) -> Option<String> {
    let name = file_name.strip_suffix(".json")?;
    valid_preset_name(name).then(|| name.to_string())
}
