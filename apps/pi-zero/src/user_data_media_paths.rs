use std::path::PathBuf;

const RECORDINGS_DIR_ENV: &str = "OCTESSERA_PI_RECORDINGS_DIR";
const SCREEN_RECORDINGS_DIR_ENV: &str = "OCTESSERA_PI_SCREEN_RECORDINGS_DIR";

pub(crate) fn recordings_dir() -> PathBuf {
    std::env::var_os(RECORDINGS_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/pi/recordings"))
}

pub(crate) fn screen_recordings_dir() -> PathBuf {
    std::env::var_os(SCREEN_RECORDINGS_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/pi/screen-recordings"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_paths_have_bounded_explicit_contracts() {
        assert_eq!(RECORDINGS_DIR_ENV, "OCTESSERA_PI_RECORDINGS_DIR");
        assert_eq!(
            SCREEN_RECORDINGS_DIR_ENV,
            "OCTESSERA_PI_SCREEN_RECORDINGS_DIR"
        );
    }
}
