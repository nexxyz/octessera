use serde::Deserialize;
use std::path::Path;

const PRODUCTION_RELEASES_ROOT: &str = "/opt/octessera/releases";
const PRODUCTION_METADATA_NAME: &str = "octessera-runtime.json";
const RUNTIME_BINARY_NAME: &str = "octessera-pi";
const ORANGE_PROFILE: &str = "orange-pi-zero-2w";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductionRuntimeMetadata {
    artifact_kind: String,
    binary_sha256: String,
    name: String,
    profile: String,
    runtime_ready: bool,
    version: String,
}

pub(super) fn validate(executable: &Path) -> Result<String, String> {
    validate_at(executable, Path::new(PRODUCTION_RELEASES_ROOT))
}

fn validate_at(executable: &Path, releases_root: &Path) -> Result<String, String> {
    let release_version = release_version(executable, releases_root)?;
    let metadata_path = executable
        .parent()
        .ok_or_else(|| {
            format!(
                "Orange runtime executable has no parent: {}",
                executable.display()
            )
        })?
        .join(PRODUCTION_METADATA_NAME);
    let metadata_text = super::support::read_small(&metadata_path)
        .map_err(|error| format!("production metadata: {error}"))?;
    let metadata = parse_metadata(&metadata_text)?;
    let executable_hash = octessera_hal::orange_metadata::hash_file(executable)?;
    validate_metadata(&metadata, &executable_hash, &release_version)?;
    Ok(metadata_text)
}

fn release_version(executable: &Path, releases_root: &Path) -> Result<String, String> {
    let relative = executable.strip_prefix(releases_root).map_err(|_| {
        format!(
            "Orange runtime executable must resolve under {}/<version>/{RUNTIME_BINARY_NAME}: {}",
            releases_root.display(),
            executable.display()
        )
    })?;
    let components = relative.components().collect::<Vec<_>>();
    if components.len() != 2
        || !matches!(components[0], std::path::Component::Normal(_))
        || components[1].as_os_str() != RUNTIME_BINARY_NAME
    {
        return Err(format!(
            "Orange runtime executable path must be {}/<version>/{RUNTIME_BINARY_NAME}: {}",
            releases_root.display(),
            executable.display()
        ));
    }
    let version = components[0]
        .as_os_str()
        .to_str()
        .ok_or_else(|| "Orange release directory name is not valid UTF-8".to_string())?;
    if version.is_empty() {
        return Err("Orange release directory name must not be empty".into());
    }
    Ok(version.into())
}

fn parse_metadata(input: &str) -> Result<ProductionRuntimeMetadata, String> {
    serde_json::from_str(input)
        .map_err(|error| format!("production metadata JSON is invalid: {error}"))
}

fn validate_metadata(
    metadata: &ProductionRuntimeMetadata,
    executable_hash: &str,
    release_version: &str,
) -> Result<(), String> {
    if metadata.artifact_kind != "production-runtime"
        || metadata.name != RUNTIME_BINARY_NAME
        || metadata.profile != ORANGE_PROFILE
        || !metadata.runtime_ready
        || metadata.version != release_version
    {
        return Err(
            "production metadata identity fields do not match the Orange runtime contract".into(),
        );
    }
    if !is_lower_hex_sha256(&metadata.binary_sha256) {
        return Err(
            "production metadata binary_sha256 must be 64 lowercase hexadecimal characters".into(),
        );
    }
    if metadata.binary_sha256 != executable_hash {
        return Err("production metadata binary_sha256 does not match the executable".into());
    }
    Ok(())
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::{validate_at, RUNTIME_BINARY_NAME};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_path() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("octessera-fat-installed-identity-{stamp}"))
    }

    fn fixture() -> (PathBuf, PathBuf, String) {
        let root = temporary_path();
        let release = root.join("1.2.3");
        fs::create_dir_all(&release).expect("release directory");
        let executable = release.join(RUNTIME_BINARY_NAME);
        fs::write(&executable, b"orange production executable").expect("runtime executable");
        let hash = octessera_hal::orange_metadata::hash_file(&executable).expect("runtime hash");
        let metadata = format!(
            "{{\"artifact_kind\":\"production-runtime\",\"binary_sha256\":\"{hash}\",\"name\":\"octessera-pi\",\"profile\":\"orange-pi-zero-2w\",\"runtime_ready\":true,\"version\":\"1.2.3\"}}"
        );
        fs::write(release.join("octessera-runtime.json"), &metadata).expect("production metadata");
        (root, executable, metadata)
    }

    #[test]
    fn valid_fixture_returns_source_metadata() {
        let (root, executable, metadata) = fixture();
        assert_eq!(validate_at(&executable, &root).unwrap(), metadata);
        fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn malformed_extra_wrong_identity_and_hash_metadata_fails() {
        type MetadataMutation = (&'static str, fn(&str) -> String);
        let mutations: [MetadataMutation; 5] = [
            ("malformed", |_metadata: &str| "{malformed".into()),
            ("extra", |metadata: &str| {
                metadata.replace('}', ",\"extra\":1}")
            }),
            ("wrong profile", |metadata: &str| {
                metadata.replace("orange-pi-zero-2w", "raspberry-pi-zero-2w")
            }),
            ("wrong version", |metadata: &str| {
                metadata.replace("\"version\":\"1.2.3\"", "\"version\":\"1.2.4\"")
            }),
            ("wrong hash", |metadata: &str| {
                metadata.replace(
                    metadata
                        .split("\"binary_sha256\":\"")
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .next()
                        .unwrap(),
                    &"0".repeat(64),
                )
            }),
        ];
        for (label, mutate) in mutations {
            let (root, executable, metadata) = fixture();
            fs::write(
                executable.parent().unwrap().join("octessera-runtime.json"),
                mutate(&metadata),
            )
            .expect("mutated metadata");
            assert!(validate_at(&executable, &root).is_err(), "{label} accepted");
            fs::remove_dir_all(root).expect("fixture cleanup");
        }
    }

    #[test]
    fn bad_installed_path_and_parent_version_mismatch_fail() {
        let (root, executable, metadata) = fixture();
        let production = executable.parent().unwrap().join("octessera-runtime.json");
        fs::write(
            &production,
            metadata.replace("\"version\":\"1.2.3\"", "\"version\":\"9.9.9\""),
        )
        .expect("wrong version metadata");
        assert!(validate_at(&executable, &root).is_err());
        assert!(validate_at(&root.join("wrong").join(RUNTIME_BINARY_NAME), &root).is_err());
        assert!(validate_at(&root.join("1.2.3").join("wrong"), &root).is_err());
        fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_metadata_fails() {
        use std::os::unix::fs::symlink;

        let (root, executable, metadata) = fixture();
        let production = executable.parent().unwrap().join("octessera-runtime.json");
        let target = root.join("target.json");
        fs::remove_file(&production).expect("production metadata");
        fs::write(&target, metadata).expect("metadata target");
        symlink(&target, &production).expect("metadata symlink");
        assert!(validate_at(&executable, &root).is_err());
        fs::remove_dir_all(root).expect("fixture cleanup");
    }
}
