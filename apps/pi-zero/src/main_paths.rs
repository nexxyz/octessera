#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::path::Path;
use std::path::PathBuf;

const STORE_DIR_ENV: &str = "OCTESSERA_PI_STORE_DIR";
const SAMPLES_DIR_ENV: &str = "OCTESSERA_PI_SAMPLES_DIR";

pub(crate) fn default_store_dir() -> PathBuf {
    configured_dir(STORE_DIR_ENV, "presets")
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn ensure_runtime_dirs(store_dir: &Path, samples_dir: &Path) {
    let _ = std::fs::create_dir_all(samples_dir);
    let _ = std::fs::create_dir_all(store_dir);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn ensure_samples_dir(samples_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(samples_dir)
        .map_err(|error| format!("Pi samples directory is not usable: {error}"))?;
    let metadata = std::fs::metadata(samples_dir)
        .map_err(|error| format!("Pi samples directory cannot be inspected: {error}"))?;
    if !metadata.is_dir() {
        return Err(format!(
            "Pi samples path is not a directory: {}",
            samples_dir.display()
        ));
    }
    Ok(())
}

pub(crate) fn default_samples_dir() -> PathBuf {
    configured_dir(SAMPLES_DIR_ENV, "samples")
}

fn configured_dir(environment_variable: &str, fallback_name: &str) -> PathBuf {
    configured_dir_from(
        std::env::var_os(environment_variable).map(PathBuf::from),
        home_dir(),
        fallback_name,
    )
}

fn configured_dir_from(
    configured: Option<PathBuf>,
    home: Option<PathBuf>,
    fallback_name: &str,
) -> PathBuf {
    configured.unwrap_or_else(|| {
        home.map(|home| home.join(fallback_name))
            .unwrap_or_else(|| PathBuf::from(fallback_name))
    })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    use super::ensure_samples_dir;
    use super::{configured_dir_from, SAMPLES_DIR_ENV, STORE_DIR_ENV};
    use std::path::PathBuf;

    #[test]
    fn orange_path_contract_uses_explicit_pi_environment_variables() {
        assert_eq!(STORE_DIR_ENV, "OCTESSERA_PI_STORE_DIR");
        assert_eq!(SAMPLES_DIR_ENV, "OCTESSERA_PI_SAMPLES_DIR");
        assert_eq!(
            configured_dir_from(
                Some(PathBuf::from("configured/presets")),
                Some(PathBuf::from("home")),
                "presets",
            ),
            PathBuf::from("configured/presets")
        );
        assert_eq!(
            configured_dir_from(
                Some(PathBuf::from("configured/samples")),
                Some(PathBuf::from("home")),
                "samples",
            ),
            PathBuf::from("configured/samples")
        );
    }

    #[test]
    fn path_contract_falls_back_to_home_only_when_unconfigured() {
        assert_eq!(
            configured_dir_from(None, Some(PathBuf::from("home")), "presets"),
            PathBuf::from("home/presets")
        );
        assert_eq!(
            configured_dir_from(None, None, "samples"),
            PathBuf::from("samples")
        );
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    #[test]
    fn sample_root_creation_failure_is_returned() {
        let root = std::env::temp_dir().join(format!(
            "octessera-pi-samples-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("samples");
        std::fs::write(&path, b"not a directory").unwrap();
        let marker = root.join("candidate-ready.json");
        let readiness = crate::candidate_readiness::CandidateReadiness::new(
            Some(marker.clone()),
            "pi-sample-root-failure".into(),
        );

        let error = ensure_samples_dir(&path).unwrap_err();

        assert!(error.contains("Pi samples directory is not usable"));
        assert!(!marker.exists());
        drop(readiness);
        let _ = std::fs::remove_dir_all(root);
    }
}
