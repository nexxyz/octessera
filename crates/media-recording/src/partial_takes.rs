use super::avi_repair::{repair_partial_avi, PARTIAL_AVI_SUFFIX};
use super::wav::{repair_partial_wav, PARTIAL_WAV_SUFFIX};
use super::RecordingOutcome;
use std::fs;
use std::path::{Path, PathBuf};

/// The `.partial.avi` and `.partial.wav` takes in `dir`; list them before any new take can
/// start in it.
pub fn partial_take_paths(dir: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.ends_with(PARTIAL_AVI_SUFFIX) || name.ends_with(PARTIAL_WAV_SUFFIX)
                    })
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

/// Saves a take a power cut interrupted as `.incomplete.avi` or `.incomplete.wav`.
pub fn repair_partial_take(partial_path: &Path) -> Result<RecordingOutcome, String> {
    if partial_path.to_string_lossy().ends_with(PARTIAL_WAV_SUFFIX) {
        repair_partial_wav(partial_path)
    } else {
        repair_partial_avi(partial_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn only_partial_takes_are_listed() {
        let dir = std::env::temp_dir().join(format!(
            "octessera-partial-takes-listing-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "b.partial.avi",
            "a.partial.avi",
            "c.avi",
            "d.partial.wav",
            "e.wav",
        ] {
            File::create(dir.join(name)).unwrap();
        }

        assert_eq!(
            partial_take_paths(&dir),
            vec![
                dir.join("a.partial.avi"),
                dir.join("b.partial.avi"),
                dir.join("d.partial.wav")
            ]
        );
        assert!(partial_take_paths(&dir.join("missing")).is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
