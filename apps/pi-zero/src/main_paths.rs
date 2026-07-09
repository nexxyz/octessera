use std::path::{Path, PathBuf};

pub(crate) fn default_store_dir() -> PathBuf {
    std::env::var_os("OCTESSERA_PI_STORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            home_dir()
                .map(|home| home.join("presets"))
                .unwrap_or_else(|| PathBuf::from("presets"))
        })
}

pub(crate) fn ensure_runtime_dirs(store_dir: &Path, samples_dir: &Path) {
    let _ = std::fs::create_dir_all(samples_dir);
    let _ = std::fs::create_dir_all(store_dir);
}

pub(crate) fn default_samples_dir() -> PathBuf {
    std::env::var_os("OCTESSERA_PI_SAMPLES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            home_dir()
                .map(|home| home.join("samples"))
                .unwrap_or_else(|| PathBuf::from("samples"))
        })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
